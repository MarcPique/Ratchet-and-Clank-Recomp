using System.Diagnostics;
using System.IO.Compression;
using System.Reflection;
using System.Text.Json;

namespace RatchetClankInstaller;

internal sealed record InstallProgress(string Message, int Percent);
internal sealed record InstallResult(string Game, string? Dlss5Warning);

internal static class InstallerCore
{
    internal const string GameName = "Ratchet & Clank.exe";
    private const string ExtractorName = "Extraer datos.exe";

    internal static async Task<InstallResult> InstallAsync(
        string isoInput, string destinationInput, bool desktopShortcut, bool installDlss5,
        IProgress<InstallProgress>? progress, CancellationToken token)
    {
        if (string.IsNullOrWhiteSpace(isoInput) || string.IsNullOrWhiteSpace(destinationInput))
            throw new InvalidOperationException("Selecciona la ISO y la carpeta donde instalar el juego.");

        string iso = Path.GetFullPath(isoInput.Trim().Trim('"'));
        string destination = Path.GetFullPath(destinationInput.Trim().Trim('"'));
        if (!File.Exists(iso) || !iso.EndsWith(".iso", StringComparison.OrdinalIgnoreCase))
            throw new InvalidOperationException("Selecciona un archivo ISO existente.");
        if (Path.GetPathRoot(destination)?.TrimEnd(Path.DirectorySeparatorChar) == destination.TrimEnd(Path.DirectorySeparatorChar))
            throw new InvalidOperationException("Elige una carpeta para el juego, no la raíz de una unidad.");
        if (Directory.Exists(destination) && Directory.EnumerateFileSystemEntries(destination).Any())
            throw new InvalidOperationException("La carpeta de destino debe estar vacía. Elige una carpeta nueva o vacía.");

        long required = new FileInfo(iso).Length + 1_500_000_000L;
        var drive = new DriveInfo(Path.GetPathRoot(destination)!);
        if (drive.AvailableFreeSpace < required)
            throw new InvalidOperationException($"Se necesitan al menos {required / 1_000_000_000.0:F1} GB libres en la unidad de destino.");

        Directory.CreateDirectory(destination);
        string stage = Path.Combine(destination, ".instalacion-" + Guid.NewGuid().ToString("N"));
        string package = Path.Combine(stage, "paquete");
        string data = Path.Combine(stage, "Data");
        var committed = new List<string>();
        try
        {
            token.ThrowIfCancellationRequested();
            progress?.Report(new("Preparando los archivos del juego…", 1));
            Directory.CreateDirectory(package);
            using (Stream bundle = Assembly.GetExecutingAssembly().GetManifestResourceStream("GameBundle.zip")
                ?? throw new InvalidOperationException("El instalador no contiene los archivos del juego."))
                ZipFile.ExtractToDirectory(bundle, package);

            string[] games = Directory.GetFiles(package, GameName, SearchOption.AllDirectories);
            if (games.Length != 1)
                throw new InvalidOperationException("El paquete del instalador está incompleto.");
            string packageRoot = Path.GetDirectoryName(games[0])!;
            string extractor = Path.Combine(packageRoot, ExtractorName);
            if (!File.Exists(extractor) || !Directory.Exists(Path.Combine(packageRoot, "assets", "shaders")))
                throw new InvalidOperationException("Faltan el extractor o los shaders del juego.");

            progress?.Report(new("Comprobando la ISO…", 3));
            await RunExtractor(extractor, ["identify", "--iso", iso, "--json"], progress, 3, 5, token);
            progress?.Report(new("Extrayendo y comprobando los datos…", 8));
            await RunExtractor(extractor, ["extract", "--iso", iso, "--out", data, "--json"], progress, 8, 87, token);
            if (!File.Exists(Path.Combine(data, "extract-info.json")) || !File.Exists(Path.Combine(data, "toc.bin")))
                throw new InvalidOperationException("La extracción no produjo todos los archivos necesarios.");
            progress?.Report(new("Verificando la instalación…", 90));
            await RunExtractor(extractor, ["verify", "--out", data, "--json"], progress, 90, 98, token);
            token.ThrowIfCancellationRequested();

            // The chosen directory was checked as empty. Move verified data only after extraction
            // and verification finish; moves stay on the same volume and are nearly instantaneous.
            foreach (string file in Directory.GetFiles(packageRoot))
            {
                string target = Path.Combine(destination, Path.GetFileName(file));
                File.Move(file, target);
                committed.Add(target);
            }
            string assetsTarget = Path.Combine(destination, "assets");
            Directory.Move(Path.Combine(packageRoot, "assets"), assetsTarget);
            committed.Add(assetsTarget);
            string dataTarget = Path.Combine(destination, "Data");
            Directory.Move(data, dataTarget);
            committed.Add(dataTarget);
            string game = Path.Combine(destination, GameName);
            string? dlss5Warning = null;
            if (installDlss5)
            {
                try
                {
                    progress?.Report(new("Instalando DLSS 5 y sus componentes…", 99));
                    await Dlss5Setup.InstallAsync(game, stage, token);
                }
                catch (OperationCanceledException) { throw; }
                catch (Exception error)
                {
                    dlss5Warning = "El juego se instaló, pero DLSS 5 no pudo completarse: " + error.Message;
                }
            }

            if (desktopShortcut)
            {
                try { CreateShortcut(game, destination); }
                catch (Exception e) { progress?.Report(new("Juego instalado; no se pudo crear el acceso directo: " + e.Message, 99)); }
            }
            progress?.Report(new(dlss5Warning ?? "Instalación terminada. Ya puedes abrir Ratchet & Clank.exe.", 100));
            return new InstallResult(game, dlss5Warning);
        }
        catch
        {
            // No pre-existing user files can be in this directory: it was empty on entry.
            foreach (string path in Enumerable.Reverse(committed))
            {
                try
                {
                    if (File.Exists(path)) File.Delete(path);
                    else if (Directory.Exists(path)) Directory.Delete(path, true);
                }
                catch { /* Report the original failure; incomplete files can be removed manually. */ }
            }
            throw;
        }
        finally
        {
            try { if (Directory.Exists(stage)) Directory.Delete(stage, true); }
            catch { /* An interrupted extraction may leave a partial staging directory. */ }
        }
    }

    private static async Task RunExtractor(
        string extractor, IReadOnlyList<string> arguments, IProgress<InstallProgress>? progress,
        int from, int to, CancellationToken token)
    {
        using var process = new Process();
        process.StartInfo = new ProcessStartInfo(extractor)
        {
            UseShellExecute = false,
            CreateNoWindow = true,
            WindowStyle = ProcessWindowStyle.Hidden,
            RedirectStandardOutput = true,
            RedirectStandardError = true,
        };
        foreach (string argument in arguments) process.StartInfo.ArgumentList.Add(argument);
        process.Start();
        using var cancel = token.Register(() => { try { if (!process.HasExited) process.Kill(true); } catch { } });
        Task<string> stderr = process.StandardError.ReadToEndAsync(token);
        string lastError = "";
        while (await process.StandardOutput.ReadLineAsync(token) is { } line)
        {
            try
            {
                using JsonDocument doc = JsonDocument.Parse(line);
                JsonElement root = doc.RootElement;
                if (!root.TryGetProperty("type", out var kind)) continue;
                if (kind.GetString() == "error" && root.TryGetProperty("message", out var message))
                    lastError = message.GetString() ?? "";
                if (kind.GetString() != "progress") continue;
                string stage = root.TryGetProperty("stage", out var stageValue) ? stageValue.GetString() ?? "" : "";
                long done = root.TryGetProperty("done", out var doneValue) ? doneValue.GetInt64() : 0;
                long total = root.TryGetProperty("total", out var totalValue) ? totalValue.GetInt64() : 0;
                int percent = total > 0 ? from + (int)Math.Round((to - from) * Math.Clamp((double)done / total, 0, 1)) : from;
                string label = stage switch
                {
                    "identify" => "Comprobando la ISO…",
                    "copy" => "Extrayendo los datos del juego…",
                    "prepare" => "Preparando los datos del juego…",
                    "verify" => "Verificando los datos del juego…",
                    _ => "Instalando…",
                };
                progress?.Report(new(label, percent));
            }
            catch (JsonException) { /* Unknown output is kept out of the UI. */ }
        }
        await process.WaitForExitAsync(token);
        string stderrText = await stderr;
        if (process.ExitCode != 0)
        {
            string detail = !string.IsNullOrWhiteSpace(lastError) ? lastError : stderrText.Trim();
            throw new InvalidOperationException($"No se pudo completar la operación con la ISO (código {process.ExitCode}). {detail}");
        }
    }

    private static void CreateShortcut(string game, string destination)
    {
        Type type = Type.GetTypeFromProgID("WScript.Shell")
            ?? throw new InvalidOperationException("Windows no permite crear accesos directos.");
        dynamic shell = Activator.CreateInstance(type)!;
        string desktop = Environment.GetFolderPath(Environment.SpecialFolder.DesktopDirectory);
        dynamic link = shell.CreateShortcut(Path.Combine(desktop, "Ratchet & Clank.lnk"));
        link.TargetPath = game;
        link.Arguments = "";
        link.Description = "Ratchet & Clank PC";
        link.WorkingDirectory = destination;
        link.IconLocation = game + ",0";
        link.Save();
    }
}
