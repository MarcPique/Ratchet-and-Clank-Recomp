using System.Diagnostics;
using System.IO.Compression;
using System.Reflection;
using System.Security.Cryptography;

namespace RatchetClankInstaller;

/// <summary>
/// Installs the feeder with the three runtimes captured from the user's working setup.
/// The local runtime archive is supplied only when building the personal installer.
/// </summary>
internal static class Dlss5Setup
{
    private const string ScriptUrl = "https://raw.githubusercontent.com/jlrouzies-fr/DLSS5-Feeder/3c09eab/tools/Install-DLSS5Feeder.ps1";
    private const string FeederUrl = "https://github.com/jlrouzies-fr/DLSS5-Feeder/releases/download/v1.19.0/DLSS5-Feeder-1.19.0.zip";
    private const string FeederSha256 = "9cc19e28f8fb4e216ea61621fdf6443e207e338ec639979ab335503c75f48f36";
    private const string DlssNrSha256 = "6eb209e764f39872625debd6abaf45e2bb6322f6f270f781f70c059ae30b3927";
    private const string DlssSha256 = "3975567b8943c53acce397f2b72380092f84f162d00b0d2c7d08a1025c563983";
    private const string RenoDxSha256 = "dcd93881e976ad033d83c2bb01f4bc3e4ddc59c15fe0dd4ca165bc5fc7d1ac68";
    internal static bool IsBundled => Assembly.GetExecutingAssembly().GetManifestResourceInfo("Dlss5Runtime.zip") is not null;

    internal static async Task InstallAsync(string game, string stage, CancellationToken token)
    {
        string tools = Path.Combine(stage, "dlss5");
        Directory.CreateDirectory(tools);
        string script = Path.Combine(tools, "Install-DLSS5Feeder.ps1");
        string feeder = Path.Combine(tools, "DLSS5-Feeder-1.19.0.zip");
        string local = Path.Combine(tools, "local");
        Directory.CreateDirectory(local);
        await using (Stream resource = Assembly.GetExecutingAssembly().GetManifestResourceStream("Dlss5Runtime.zip")
            ?? throw new InvalidOperationException("Esta compilación no contiene el runtime DLSS 5 personal."))
        await using (var archive = File.Create(Path.Combine(tools, "Dlss5Runtime.zip")))
            await resource.CopyToAsync(archive, token);
        ZipFile.ExtractToDirectory(Path.Combine(tools, "Dlss5Runtime.zip"), local);
        await Verify(local, "nvngx_dlssnr.dll", DlssNrSha256, token);
        await Verify(local, "nvngx_dlss.dll", DlssSha256, token);
        await Verify(local, "renodx-dlss5.addon64", RenoDxSha256, token);
        using var http = new HttpClient { Timeout = TimeSpan.FromMinutes(10) };
        http.DefaultRequestHeaders.UserAgent.ParseAdd("RatchetClankInstaller/1.3.0");
        await Download(http, ScriptUrl, script, token);
        await Download(http, FeederUrl, feeder, token);
        await using var downloadedFeeder = File.OpenRead(feeder);
        string hash = Convert.ToHexString(await SHA256.HashDataAsync(downloadedFeeder, token));
        if (!hash.Equals(FeederSha256, StringComparison.OrdinalIgnoreCase))
            throw new InvalidOperationException("La descarga del feeder no coincide con el SHA-256 publicado por su autor.");

        using var process = new Process();
        process.StartInfo = new ProcessStartInfo("powershell.exe")
        {
            UseShellExecute = false,
            CreateNoWindow = true,
            WindowStyle = ProcessWindowStyle.Hidden,
            RedirectStandardOutput = true,
            RedirectStandardError = true,
        };
        foreach (string argument in new[] {
            "-NoProfile", "-ExecutionPolicy", "Bypass", "-File", script,
            "-GameExe", game, "-Api", "Vulkan", "-Consumer", "RenoDX",
            "-FeederZip", feeder, "-LocalFiles", local,
            "-DlssNrDll", Path.Combine(local, "nvngx_dlssnr.dll"),
            "-DlssDll", Path.Combine(local, "nvngx_dlss.dll"),
            "-RenoDxAddon", Path.Combine(local, "renodx-dlss5.addon64"),
            "-Yes", "-NoPause",
        }) process.StartInfo.ArgumentList.Add(argument);
        process.Start();
        using var cancel = token.Register(() => { try { if (!process.HasExited) process.Kill(true); } catch { } });
        Task<string> stdout = process.StandardOutput.ReadToEndAsync(token);
        Task<string> stderr = process.StandardError.ReadToEndAsync(token);
        await process.WaitForExitAsync(token);
        string outputText = await stdout;
        string errorText = await stderr;
        string details = (outputText + "\n" + errorText).Trim();
        string folder = Path.GetDirectoryName(game)!;
        if (process.ExitCode != 0)
        {
            File.WriteAllText(Path.Combine(folder, "dlss5-install.log"), details);
            throw new InvalidOperationException($"El instalador oficial del feeder devolvió {process.ExitCode}. {details[^Math.Min(400, details.Length)..]}");
        }

        foreach (string name in new[] { "dlss5-feed.addon64", "renodx-dlss5.addon64", "nvngx_dlssnr.dll", "nvngx_dlss.dll", "ReShade.ini", "ReShadePreset.ini" })
            if (!File.Exists(Path.Combine(folder, name)))
                throw new InvalidOperationException("La instalación de DLSS 5 no produjo " + name + ".");

        // The upstream installer does not create this file until first launch. Our game
        // needs it before Bevy chooses Vulkan and discovers the native menu switch.
        string cfg = Path.Combine(folder, "dlss5-feed.cfg");
        if (!File.Exists(cfg)) File.WriteAllText(cfg, "enabled=1\nmode=0\npreset=0\nrc_profile=0\n");
        SetSetting(cfg, null, "enabled", "1");
        SetSetting(cfg, null, "mode", "0");
        SetSetting(cfg, null, "rc_profile", "0");
        SetSetting(cfg, null, "preset", "0");
        string ini = Path.Combine(folder, "ReShade.ini");
        SetSetting(ini, "OVERLAY", "TutorialProgress", "4");
        SetSetting(ini, "OVERLAY", "ShowFPS", "0");
        SetSetting(ini, "OVERLAY", "ShowFrameTime", "0");
        SetSetting(ini, "OVERLAY", "ShowClock", "0");
        SetSetting(ini, "OVERLAY", "ShowPresetName", "0");
        SetSetting(ini, "OVERLAY", "ShowScreenshotMessage", "0");
        SetSetting(ini, "INPUT", "KeyOverlay", "0,0,0,0");
    }

    private static async Task Download(HttpClient http, string url, string path, CancellationToken token)
    {
        using var response = await http.GetAsync(url, HttpCompletionOption.ResponseHeadersRead, token);
        response.EnsureSuccessStatusCode();
        await using Stream input = await response.Content.ReadAsStreamAsync(token);
        await using Stream output = File.Create(path);
        await input.CopyToAsync(output, token);
    }

    private static async Task Verify(string folder, string name, string expected, CancellationToken token)
    {
        string path = Path.Combine(folder, name);
        if (!File.Exists(path)) throw new InvalidOperationException("Falta el runtime " + name + ".");
        await using var file = File.OpenRead(path);
        string actual = Convert.ToHexString(await SHA256.HashDataAsync(file, token));
        if (!actual.Equals(expected, StringComparison.OrdinalIgnoreCase))
            throw new InvalidOperationException("El runtime " + name + " no coincide con la instalación comprobada.");
    }

    private static void SetSetting(string path, string? section, string key, string value)
    {
        var lines = File.ReadAllLines(path).ToList();
        int sectionStart = 0;
        int sectionEnd = lines.Count;
        if (section is not null)
        {
            sectionStart = lines.FindIndex(line => line.Trim().Equals($"[{section}]", StringComparison.OrdinalIgnoreCase));
            if (sectionStart < 0)
            {
                lines.Add($"[{section}]");
                sectionStart = lines.Count - 1;
            }
            sectionStart++;
            sectionEnd = lines.FindIndex(sectionStart, line => line.TrimStart().StartsWith('['));
            if (sectionEnd < 0) sectionEnd = lines.Count;
        }
        for (int i = sectionStart; i < sectionEnd; i++)
        {
            int equals = lines[i].IndexOf('=');
            if (equals < 0 || !lines[i][..equals].Trim().Equals(key, StringComparison.OrdinalIgnoreCase)) continue;
            lines[i] = key + "=" + value;
            File.WriteAllLines(path, lines);
            return;
        }
        lines.Insert(sectionEnd, key + "=" + value);
        File.WriteAllLines(path, lines);
    }
}
