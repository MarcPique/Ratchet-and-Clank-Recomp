using System.Globalization;
using System.Text;
using System.Text.RegularExpressions;

namespace RatchetClankInstaller;

internal sealed record GameLanguage(string Code, string Name, uint Slot)
{
    internal static readonly GameLanguage[] All =
    [
        new("en", "English", 0),
        new("fr", "Français", 2),
        new("de", "Deutsch", 3),
        new("es", "Español", 4),
        new("it", "Italiano", 5),
    ];

    internal static GameLanguage FromCode(string? code) =>
        All.FirstOrDefault(x => x.Code.Equals(code, StringComparison.OrdinalIgnoreCase))
        ?? throw new ArgumentException("Idioma no válido. Usa en, fr, de, es o it.");

    internal static GameLanguage SystemDefault()
    {
        try
        {
            string file = SettingsPath();
            if (File.Exists(file))
            {
                Match match = Regex.Match(File.ReadAllText(file), @"(?m)^\s*language\s*=\s*(\d+)\s*$", RegexOptions.None, TimeSpan.FromSeconds(1));
                if (match.Success && uint.TryParse(match.Groups[1].Value, out uint slot))
                {
                    GameLanguage? saved = All.FirstOrDefault(x => x.Slot == slot);
                    if (saved is not null) return saved;
                }
            }
        }
        catch (IOException) { /* Fall back to the Windows UI language. */ }
        catch (UnauthorizedAccessException) { /* Fall back to the Windows UI language. */ }
        return All.FirstOrDefault(x => x.Code == CultureInfo.CurrentUICulture.TwoLetterISOLanguageName) ?? All[0];
    }

    public override string ToString() => Name;

    internal void SaveStartupLanguage()
    {
        // The game reads this exact key before loading its first menu and audio bank.
        string file = SettingsPath();
        Directory.CreateDirectory(Path.GetDirectoryName(file)!);
        string original = File.Exists(file) ? File.ReadAllText(file) : "";
        string replacement = "language = " + Slot.ToString(CultureInfo.InvariantCulture);
        string updated = Regex.IsMatch(original, @"(?m)^\s*language\s*=")
            ? Regex.Replace(original, @"(?m)^\s*language\s*=.*$", replacement, RegexOptions.None, TimeSpan.FromSeconds(1))
            : original.TrimEnd('\r', '\n') + (original.Length > 0 ? Environment.NewLine : "") + replacement + Environment.NewLine;
        string temp = file + ".installer-" + Guid.NewGuid().ToString("N");
        try
        {
            File.WriteAllText(temp, updated, new UTF8Encoding(false));
            File.Move(temp, file, true);
        }
        finally
        {
            if (File.Exists(temp)) File.Delete(temp);
        }
    }

    private static string SettingsPath()
    {
        string? overridePath = Environment.GetEnvironmentVariable("RC_SETTINGS_FILE");
        return !string.IsNullOrWhiteSpace(overridePath) && overridePath != "0"
            ? Path.GetFullPath(overridePath)
            : Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData), "ratchet_clank", "settings.toml");
    }
}
