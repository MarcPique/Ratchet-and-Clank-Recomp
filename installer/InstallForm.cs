using System.Diagnostics;
using System.Drawing.Drawing2D;
using System.Reflection;
using System.Runtime.InteropServices;
using System.Windows.Forms;

namespace RatchetClankInstaller;

internal sealed class InstallForm : Form
{
    private readonly Panel shell = new();
    private readonly BrandPanel brand = new();
    private readonly Panel content = new();
    private readonly PictureBox logo = new();
    private readonly Label brandEyebrow = new();
    private readonly Label brandHeading = new();
    private readonly Label brandCopy = new();
    private readonly Label brandFooter = new();
    private readonly Label eyebrow = new();
    private readonly Label heading = new();
    private readonly Label subtitle = new();
    private readonly Label isoLabel = new();
    private readonly Label destinationLabel = new();
    private readonly TextBox iso = new();
    private readonly TextBox destination = new();
    private readonly Button browseIso = new();
    private readonly Button browseDestination = new();
    private readonly Button install = new();
    private readonly Button cancel = new();
    private readonly Button play = new();
    private readonly CheckBox shortcut = new();
    private readonly CheckBox dlss5 = new();
    private readonly ProgressMeter meter = new();
    private readonly Label status = new();
    private readonly Panel divider = new();
    private CancellationTokenSource? running;
    private string? installedGame;

    private static readonly Color Night = Color.FromArgb(10, 16, 27);
    private static readonly Color PanelColor = Color.FromArgb(19, 28, 42);
    private static readonly Color Field = Color.FromArgb(31, 43, 59);
    private static readonly Color White = Color.FromArgb(244, 247, 251);
    private static readonly Color Muted = Color.FromArgb(174, 190, 207);
    private static readonly Color Orange = Color.FromArgb(242, 150, 64);

    [DllImport("dwmapi.dll")]
    private static extern int DwmSetWindowAttribute(IntPtr window, int attribute, ref int value, int size);

    internal InstallForm(bool showcase = false)
    {
        AutoScaleMode = AutoScaleMode.None;
        Text = "Instalar Ratchet & Clank Recomp";
        StartPosition = FormStartPosition.CenterScreen;
        BackColor = Night;
        ForeColor = White;
        ClientSize = new Size(U(1120), U(700));
        MinimumSize = new Size(U(870), U(690));
        Font = new Font("Segoe UI", 10);
        Icon = Icon.ExtractAssociatedIcon(Application.ExecutablePath);
        DoubleBuffered = true;
        SetStyle(ControlStyles.ResizeRedraw, true);

        shell.BackColor = PanelColor;
        Controls.Add(shell);
        shell.Controls.Add(brand);
        shell.Controls.Add(content);
        content.BackColor = PanelColor;

        using (Stream stream = Assembly.GetExecutingAssembly().GetManifestResourceStream("InstallerLogo.png")
            ?? throw new InvalidOperationException("Falta el logo del instalador."))
        using (var source = Image.FromStream(stream)) logo.Image = new Bitmap(source);
        logo.SizeMode = PictureBoxSizeMode.Zoom;
        logo.BackColor = Color.Transparent;
        brand.Controls.Add(logo);

        ConfigureLabel(brandEyebrow, "RATCHET & CLANK  /  PC", 9, Orange, true);
        ConfigureLabel(brandHeading, "TU AVENTURA\nEMPIEZA AQUÍ", 19, White, true);
        ConfigureLabel(brandCopy, "Elige tu ISO original. Nosotros preparamos el resto.", 11, Color.FromArgb(224, 230, 238));
        ConfigureLabel(brandFooter, "BASADO EN ReRAC  ·  USO PERSONAL", 8, Color.FromArgb(220, 227, 237));
        foreach (var label in new[] { brandEyebrow, brandHeading, brandCopy, brandFooter }) brand.Controls.Add(label);

        ConfigureLabel(eyebrow, "INSTALACIÓN EN 2 PASOS", 9, Orange, true);
        ConfigureLabel(heading, "Prepara tu aventura", 25, White, true);
        ConfigureLabel(subtitle, "Selecciona el disco del juego y dónde quieres instalarlo.", 10, Muted);
        ConfigureLabel(isoLabel, "01   ISO DEL JUEGO", 10, White, true);
        ConfigureLabel(destinationLabel, "02   CARPETA DE DESTINO", 10, White, true);
        ConfigureLabel(status, "Selecciona una ISO y una carpeta nueva o vacía.", 10, Muted);
        foreach (var label in new[] { eyebrow, heading, subtitle, isoLabel, destinationLabel, status }) content.Controls.Add(label);

        divider.BackColor = Color.FromArgb(55, 70, 88);
        content.Controls.Add(divider);
        ConfigureField(iso, "Ruta a tu archivo .iso");
        ConfigureField(destination, "Carpeta nueva o vacía");
        destination.Text = showcase ? @"C:\Juegos\Ratchet & Clank PC" :
            Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.MyDocuments), "Ratchet & Clank PC");
        content.Controls.Add(iso);
        content.Controls.Add(destination);
        ConfigureButton(browseIso, "Elegir ISO", false);
        ConfigureButton(browseDestination, "Elegir carpeta", false);
        browseIso.Click += BrowseIso;
        browseDestination.Click += BrowseDestination;
        content.Controls.Add(browseIso);
        content.Controls.Add(browseDestination);

        shortcut.Text = "Crear acceso directo en el escritorio";
        shortcut.Checked = true;
        shortcut.ForeColor = White;
        shortcut.BackColor = PanelColor;
        shortcut.FlatStyle = FlatStyle.Flat;
        shortcut.Font = new Font("Segoe UI", 10);
        content.Controls.Add(shortcut);

        dlss5.Text = "Instalar DLSS 5 (RTX · requiere Internet)";
        dlss5.Checked = Dlss5Setup.IsBundled;
        dlss5.Enabled = Dlss5Setup.IsBundled;
        dlss5.ForeColor = White;
        dlss5.BackColor = PanelColor;
        dlss5.FlatStyle = FlatStyle.Flat;
        dlss5.Font = new Font("Segoe UI", 10);
        content.Controls.Add(dlss5);

        content.Controls.Add(meter);
        ConfigureButton(install, "Instalar juego", true);
        ConfigureButton(cancel, "Cerrar", false);
        ConfigureButton(play, "Jugar", false);
        install.Click += InstallClicked;
        cancel.Click += (_, _) => { if (running is null) Close(); else running.Cancel(); };
        play.Enabled = false;
        play.Click += (_, _) =>
        {
            if (installedGame is not null)
                Process.Start(new ProcessStartInfo(installedGame) { WorkingDirectory = Path.GetDirectoryName(installedGame)!, UseShellExecute = true });
        };
        content.Controls.Add(install);
        content.Controls.Add(cancel);
        content.Controls.Add(play);

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
        FormClosing += (_, e) =>
        {
            if (running is null) return;
            running.Cancel();
            e.Cancel = true;
        };
        Resize += (_, _) => LayoutUi();
        DpiChanged += (_, _) => BeginInvoke(LayoutUi);
        LayoutUi();
    }

    private int U(int value) => (int)Math.Round(value * DeviceDpi / 96.0);

    protected override void OnHandleCreated(EventArgs e)
    {
        base.OnHandleCreated(e);
        if (OperatingSystem.IsWindowsVersionAtLeast(10, 0, 17763))
        {
            int dark = 1;
            DwmSetWindowAttribute(Handle, 20, ref dark, sizeof(int));
        }
    }

    private static void ConfigureLabel(Label label, string value, float points, Color color, bool bold = false)
    {
        label.Text = value;
        label.Font = new Font("Segoe UI", points, bold ? FontStyle.Bold : FontStyle.Regular);
        label.ForeColor = color;
        label.BackColor = Color.Transparent;
        label.AutoSize = false;
    }

    private static void ConfigureField(TextBox field, string placeholder)
    {
        field.BackColor = Field;
        field.ForeColor = White;
        field.BorderStyle = BorderStyle.FixedSingle;
        field.Font = new Font("Segoe UI", 11);
        field.PlaceholderText = placeholder;
    }

    private static void ConfigureButton(Button button, string value, bool primary)
    {
        button.Text = value;
        button.Font = new Font("Segoe UI Semibold", 10, FontStyle.Bold);
        button.BackColor = primary ? Orange : Field;
        button.ForeColor = primary ? Night : White;
        button.FlatStyle = FlatStyle.Flat;
        button.FlatAppearance.BorderSize = 0;
        button.UseVisualStyleBackColor = false;
        button.Cursor = Cursors.Hand;
    }

    private void LayoutUi()
    {
        int margin = U(26);
        int width = Math.Min(U(1120), Math.Max(0, ClientSize.Width - margin * 2));
        int height = Math.Min(U(700), Math.Max(0, ClientSize.Height - margin * 2));
        shell.SetBounds((ClientSize.Width - width) / 2, (ClientSize.Height - height) / 2, width, height);

        int left = Math.Clamp((int)(width * .34), U(250), U(380));
        brand.SetBounds(0, 0, left, height);
        content.SetBounds(left, 0, width - left, height);
        int bp = U(28);
        brandEyebrow.SetBounds(bp, U(28), left - bp * 2, U(30));
        logo.SetBounds(U(17), U(103), left - U(34), U(224));
        brandHeading.SetBounds(bp, U(350), left - bp * 2, U(75));
        brandCopy.SetBounds(bp, U(437), left - bp * 2, U(76));
        brandFooter.SetBounds(bp, height - U(40), left - bp * 2, U(23));

        int pad = U(40);
        int right = content.Width - pad;
        int usable = right - pad;
        eyebrow.SetBounds(pad, U(40), usable, U(24));
        heading.SetBounds(pad, U(69), usable, U(50));
        subtitle.SetBounds(pad, U(123), usable, U(44));
        divider.SetBounds(pad, U(180), usable, U(1));
        isoLabel.SetBounds(pad, U(210), usable, U(29));
        destinationLabel.SetBounds(pad, U(326), usable, U(29));
        int browseWidth = U(126);
        int gap = U(12);
        int fieldWidth = Math.Max(U(150), usable - browseWidth - gap);
        iso.SetBounds(pad, U(246), fieldWidth, U(40));
        browseIso.SetBounds(pad + fieldWidth + gap, U(244), browseWidth, U(44));
        destination.SetBounds(pad, U(362), fieldWidth, U(40));
        browseDestination.SetBounds(pad + fieldWidth + gap, U(360), browseWidth, U(44));
        int footerY = content.Height - U(116);
        int meterY = footerY - U(48);
        shortcut.SetBounds(pad, meterY - U(82), usable, U(32));
        dlss5.SetBounds(pad, meterY - U(47), usable, U(32));
        meter.SetBounds(pad, meterY, usable, U(8));
        status.SetBounds(pad, footerY - U(31), usable, U(39));
        int buttonY = content.Height - U(71);
        install.SetBounds(pad, buttonY, U(150), U(45));
        cancel.SetBounds(pad + U(162), buttonY, U(108), U(45));
        play.SetBounds(right - U(108), buttonY, U(108), U(45));
    }

    protected override void OnPaintBackground(PaintEventArgs e)
    {
        if (ClientSize.Width <= 0 || ClientSize.Height <= 0) return;
        using var background = new LinearGradientBrush(ClientRectangle, Color.FromArgb(8, 15, 27), Color.FromArgb(19, 35, 51), 25f);
        e.Graphics.FillRectangle(background, ClientRectangle);
        using var glow = new SolidBrush(Color.FromArgb(22, 242, 150, 64));
        e.Graphics.FillEllipse(glow, ClientSize.Width / 2 - U(600), -U(340), U(1200), U(700));
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
        iso.Enabled = destination.Enabled = browseIso.Enabled = browseDestination.Enabled = shortcut.Enabled = dlss5.Enabled = install.Enabled = false;
        cancel.Text = "Cancelar";
        var progress = new Progress<InstallProgress>(p =>
        {
            meter.Percent = p.Percent;
            status.Text = p.Message;
        });
        try
        {
            InstallResult result = await InstallerCore.InstallAsync(iso.Text, destination.Text, shortcut.Checked, dlss5.Checked, progress, running.Token);
            installedGame = result.Game;
            play.Enabled = true;
            MessageBox.Show(this, result.Dlss5Warning ?? "El juego está instalado y listo para abrir.",
                "Instalación terminada", MessageBoxButtons.OK,
                result.Dlss5Warning is null ? MessageBoxIcon.Information : MessageBoxIcon.Warning);
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
            dlss5.Enabled = Dlss5Setup.IsBundled;
            cancel.Text = "Cerrar";
        }
    }

    private sealed class BrandPanel : Panel
    {
        internal BrandPanel() => DoubleBuffered = true;
        protected override void OnPaintBackground(PaintEventArgs e)
        {
            if (ClientSize.Width <= 0 || ClientSize.Height <= 0) return;
            using var gradient = new LinearGradientBrush(ClientRectangle, Color.FromArgb(75, 44, 37), Color.FromArgb(29, 43, 58), 65f);
            e.Graphics.FillRectangle(gradient, ClientRectangle);
            using var top = new SolidBrush(Orange);
            e.Graphics.FillRectangle(top, 0, 0, Width, Math.Max(3, DeviceDpi / 24));
            using var line = new Pen(Color.FromArgb(100, 131, 136, 143));
            e.Graphics.DrawLine(line, Width - 1, 0, Width - 1, Height);
        }
    }

    private sealed class ProgressMeter : Control
    {
        private int percent;
        internal int Percent { get => percent; set { percent = Math.Clamp(value, 0, 100); Invalidate(); } }
        internal ProgressMeter() => DoubleBuffered = true;
        protected override void OnPaint(PaintEventArgs e)
        {
            e.Graphics.Clear(Field);
            if (percent > 0)
            {
                using var fill = new SolidBrush(Orange);
                e.Graphics.FillRectangle(fill, 0, 0, Width * percent / 100, Height);
            }
        }
    }
}
