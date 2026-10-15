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
            string? Value(string flag)
            {
                int i = Array.FindIndex(args, a => a.Equals(flag, StringComparison.OrdinalIgnoreCase));
                return i >= 0 && i + 1 < args.Length ? args[i + 1] : null;
            }
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
        if ((args.Length == 2 || args.Length == 4) && args[0] == "--preview")
        {
            using var preview = new InstallForm(true) { ShowInTaskbar = false, StartPosition = FormStartPosition.Manual, Location = new Point(-20000, -20000) };
            if (args.Length == 4) preview.ClientSize = new Size(int.Parse(args[2]), int.Parse(args[3]));
            preview.Show();
            Application.DoEvents();
            using var bitmap = new Bitmap(preview.Width, preview.Height);
            preview.DrawToBitmap(bitmap, new Rectangle(Point.Empty, bitmap.Size));
            bitmap.Save(args[1]);
            return 0;
        }
        Application.Run(new InstallForm());
        return 0;
    }
}
