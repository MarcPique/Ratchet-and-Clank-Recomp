using System.Diagnostics;
using System.Drawing;
using System.Windows.Forms;

namespace RatchetClankInstaller;

internal static class Program
{
    [STAThread]
    private static int Main(string[] args)
    {
        if (args.Contains("--silent", StringComparer.OrdinalIgnoreCase))
        {
            string? Value(string flag) => Array.IndexOf(args, flag) is int i && i >= 0 && i + 1 < args.Length ? args[i + 1] : null;
            string? log = Value("--log");
            try
            {
                string game = InstallerCore.InstallAsync(Value("--iso") ?? "", Value("--out") ?? "",
                    !args.Contains("--no-shortcut", StringComparer.OrdinalIgnoreCase), null, CancellationToken.None)
                    .GetAwaiter().GetResult();
                if (log is not null) File.WriteAllText(log, "OK: " + game);
                return 0;
            }
            catch (Exception error)
            {
                if (log is not null) File.WriteAllText(log, error.ToString());
                return 1;
            }
        }
        ApplicationConfiguration.Initialize();
        Application.Run(new InstallForm());
        return 0;
    }
}

internal sealed class InstallForm : Form
{
    private readonly TextBox iso = new();
    private readonly TextBox destination = new();
    private readonly Button browseIso = new();
    private readonly Button browseDestination = new();
    private readonly Button install = new();
    private readonly Button cancel = new();
    private readonly Button play = new();
    private readonly CheckBox shortcut = new();
    private readonly ProgressBar bar = new();
    private readonly Label status = new();
    private CancellationTokenSource? running;
    private string? installedGame;

    private static readonly Color Background = Color.FromArgb(18, 24, 34);
    private static readonly Color Field = Color.FromArgb(31, 41, 55);
    private static readonly Color TextColor = Color.FromArgb(237, 242, 248);
    private static readonly Color Muted = Color.FromArgb(174, 188, 204);
    private static readonly Color Accent = Color.FromArgb(220, 134, 58);

    internal InstallForm()
    {
        Text = "Instalar Ratchet & Clank Recomp";
        ClientSize = new Size(760, 445);
        MinimumSize = new Size(700, 470);
        StartPosition = FormStartPosition.CenterScreen;
        BackColor = Background;
        ForeColor = TextColor;
        Font = new Font("Segoe UI", 10);
        Icon = Icon.ExtractAssociatedIcon(Application.ExecutablePath);
        AllowDrop = true;
        DragEnter += (_, e) =>
        {
            if (e.Data?.GetData(DataFormats.FileDrop) is string[] files && files.Length == 1 && files[0].EndsWith(".iso", StringComparison.OrdinalIgnoreCase))
                e.Effect = DragDropEffects.Copy;
        };
        DragDrop += (_, e) =>
        {
            if (e.Data?.GetData(DataFormats.FileDrop) is string[] files && files.Length == 1) iso.Text = files[0];
        };

        var heading = Label("Ratchet & Clank Recomp", new Font("Segoe UI Semibold", 21), TextColor);
        heading.SetBounds(34, 24, 680, 42);
        Controls.Add(heading);
        var subtitle = Label("Instala el juego usando tu propia ISO de Ratchet & Clank (USA).", Font, Muted);
        subtitle.SetBounds(36, 72, 690, 28);
        Controls.Add(subtitle);

        AddField("ISO del juego", iso, browseIso, 116, "Elegir ISO…", BrowseIso);
        AddField("Carpeta de destino", destination, browseDestination, 207, "Elegir carpeta…", BrowseDestination);
        destination.Text = Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.MyDocuments), "Ratchet & Clank PC");

        shortcut.Text = "Crear acceso directo en el escritorio";
        shortcut.Checked = true;
        shortcut.ForeColor = TextColor;
        shortcut.AutoSize = true;
        shortcut.BackColor = Background;
        shortcut.Location = new Point(35, 290);
        Controls.Add(shortcut);

        bar.Minimum = 0;
        bar.Maximum = 100;
        bar.SetBounds(35, 325, 690, 15);
        Controls.Add(bar);
        status.Text = "Selecciona la ISO y una carpeta vacía.";
        status.ForeColor = Muted;
        status.SetBounds(36, 348, 690, 26);
        Controls.Add(status);

        install.Text = "Instalar";
        StyleButton(install, Accent, Color.Black);
        install.SetBounds(35, 389, 136, 38);
        install.Click += InstallClicked;
        Controls.Add(install);

        cancel.Text = "Cerrar";
        StyleButton(cancel, Field, TextColor);
        cancel.SetBounds(184, 389, 136, 38);
        cancel.Click += (_, _) => { if (running is null) Close(); else running.Cancel(); };
        Controls.Add(cancel);

        play.Text = "Jugar";
        StyleButton(play, Field, TextColor);
        play.SetBounds(589, 389, 136, 38);
        play.Enabled = false;
        play.Click += (_, _) =>
        {
            if (installedGame is not null)
                Process.Start(new ProcessStartInfo(installedGame) { WorkingDirectory = Path.GetDirectoryName(installedGame)!, UseShellExecute = true });
        };
        Controls.Add(play);
        FormClosing += (_, e) =>
        {
            if (running is null) return;
            running.Cancel();
            e.Cancel = true;
        };
    }

    private static Label Label(string text, Font font, Color color) => new()
    {
        Text = text, Font = font, ForeColor = color, BackColor = Background,
    };

    private static void StyleButton(Button button, Color back, Color fore)
    {
        button.BackColor = back;
        button.ForeColor = fore;
        button.FlatStyle = FlatStyle.Flat;
        button.FlatAppearance.BorderSize = 0;
        button.Cursor = Cursors.Hand;
    }

    private void AddField(string title, TextBox field, Button button, int top, string buttonText, EventHandler action)
    {
        var label = Label(title, new Font("Segoe UI Semibold", 10), TextColor);
        label.SetBounds(36, top, 650, 24);
        Controls.Add(label);
        field.SetBounds(35, top + 30, 540, 34);
        field.BackColor = Field;
        field.ForeColor = TextColor;
        field.BorderStyle = BorderStyle.FixedSingle;
        Controls.Add(field);
        button.Text = buttonText;
        StyleButton(button, Field, TextColor);
        button.SetBounds(589, top + 28, 136, 37);
        button.Click += action;
        Controls.Add(button);
    }

    private void BrowseIso(object? sender, EventArgs e)
    {
        using var picker = new OpenFileDialog
        {
            Title = "Selecciona tu ISO de Ratchet & Clank (USA)",
            Filter = "Imagen de disco (*.iso)|*.iso",
            CheckFileExists = true,
        };
        if (picker.ShowDialog(this) == DialogResult.OK) iso.Text = picker.FileName;
    }

    private void BrowseDestination(object? sender, EventArgs e)
    {
        using var picker = new FolderBrowserDialog
        {
            Description = "Selecciona una carpeta nueva o vacía para el juego",
            ShowNewFolderButton = true,
            UseDescriptionForTitle = true,
        };
        if (Directory.Exists(destination.Text)) picker.SelectedPath = destination.Text;
        if (picker.ShowDialog(this) == DialogResult.OK) destination.Text = picker.SelectedPath;
    }

    private async void InstallClicked(object? sender, EventArgs e)
    {
        if (running is not null) return;
        running = new CancellationTokenSource();
        iso.Enabled = destination.Enabled = browseIso.Enabled = browseDestination.Enabled = shortcut.Enabled = install.Enabled = false;
        cancel.Text = "Cancelar";
        var progress = new Progress<InstallProgress>(p =>
        {
            bar.Value = Math.Clamp(p.Percent, 0, 100);
            status.Text = p.Message;
        });
        try
        {
            installedGame = await InstallerCore.InstallAsync(iso.Text, destination.Text, shortcut.Checked, progress, running.Token);
            play.Enabled = true;
            MessageBox.Show(this, "El juego está instalado y listo para abrir.", "Instalación terminada", MessageBoxButtons.OK, MessageBoxIcon.Information);
        }
        catch (OperationCanceledException)
        {
            status.Text = "Instalación cancelada.";
        }
        catch (Exception error)
        {
            status.Text = "No se pudo completar la instalación.";
            MessageBox.Show(this, error.Message, "Error de instalación", MessageBoxButtons.OK, MessageBoxIcon.Error);
        }
        finally
        {
            running.Dispose();
            running = null;
            iso.Enabled = destination.Enabled = browseIso.Enabled = browseDestination.Enabled = shortcut.Enabled = install.Enabled = true;
            cancel.Text = "Cerrar";
        }
    }
}
