// NotaireDZ setup launcher.
//
// The payload (program, icon, manual, uninstall script, shortcut script) is embedded as a zip
// resource. The launcher performs the install itself instead of shelling out to a batch file:
//   * copies the files to %LOCALAPPDATA%\Programs\NotaireDZ (per user, no administrator rights),
//   * runs shortcuts.ps1 to create the Desktop + Start Menu shortcuts,
//   * writes the HKCU uninstall entry so the app appears in "Add or remove programs".
// Every step is written to %TEMP%\NotaireDZ-setup.log so a failed install can be diagnosed.
using System;
using System.Diagnostics;
using System.IO;
using System.IO.Compression;
using System.Reflection;
using System.Text;
using System.Windows.Forms;
using Microsoft.Win32;

static class Program
{
    private const string Payload = "payload.zip";
    private const string Title = "NotaireDZ 1.0.1";
    private const string Version = "1.0.1";
    private static readonly string[] Files =
    {
        "NotaireDZ.exe", "NotaireDZ.ico", "NotaireDZ-Manuel.pdf", "uninstall.cmd", "shortcuts.ps1"
    };

    private static readonly StringBuilder Log = new StringBuilder();

    [STAThread]
    private static int Main(string[] args)
    {
        bool quiet = Array.IndexOf(args, "-quiet") >= 0;
        string work = Path.Combine(Path.GetTempPath(), "NotaireDZ-Setup-" + Guid.NewGuid().ToString("N").Substring(0, 8));
        string target = Target();
        try
        {
            Note("start version=" + Version + " target=" + target);

            Directory.CreateDirectory(work);
            using (Stream resource = Assembly.GetExecutingAssembly().GetManifestResourceStream(Payload))
            {
                if (resource == null)
                {
                    return Fail("Le paquet d'installation est incomplet (payload manquant).", quiet);
                }
                using (var archive = new ZipArchive(resource, ZipArchiveMode.Read))
                {
                    archive.ExtractToDirectory(work);
                }
            }
            Note("payload extracted to " + work);

            Directory.CreateDirectory(target);
            foreach (string name in Files)
            {
                string source = Path.Combine(work, name);
                string destination = Path.Combine(target, name);
                File.Copy(source, destination, true);
                Note("copied " + name + " (" + new FileInfo(destination).Length + " bytes)");
            }

            if (Environment.GetEnvironmentVariable("NOTAIRE_NO_SHORTCUTS") != "1")
            {
                Note("shortcuts: " + Run("powershell",
                    "-NoProfile -ExecutionPolicy Bypass -File \"" + Path.Combine(target, "shortcuts.ps1")
                        + "\" -Target \"" + target + "\"", target));
            }

            WriteUninstallEntry(target);
            Note("uninstall entry written");

            if (!quiet)
            {
                MessageBox.Show(
                    "NotaireDZ " + Version + " est installe.\nUn raccourci vous attend sur le Bureau.\n\n"
                        + "Identification : admin / admin\n"
                        + "Mode demonstration : raccourci \"NotaireDZ (demo)\".",
                    Title,
                    MessageBoxButtons.OK,
                    MessageBoxIcon.Information);
            }
            return 0;
        }
        catch (Exception error)
        {
            Note("FAILED: " + error);
            return Fail(error.Message, quiet);
        }
        finally
        {
            Flush();
            try
            {
                Directory.Delete(work, true);
            }
            catch
            {
                // A file can still be locked for a moment; the temp folder is harmless.
            }
        }
    }

    /// <summary>Install location: the documented per-user folder, or an override for testing.</summary>
    private static string Target()
    {
        string overridden = Environment.GetEnvironmentVariable("NOTAIRE_TARGET");
        if (!string.IsNullOrEmpty(overridden))
        {
            return overridden;
        }
        return Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData),
            "Programs", "NotaireDZ");
    }

    private static void WriteUninstallEntry(string target)
    {
        using (RegistryKey key = Registry.CurrentUser.CreateSubKey(
                   @"Software\Microsoft\Windows\CurrentVersion\Uninstall\NotaireDZ", true))
        {
            key.SetValue("DisplayName", "NotaireDZ " + Version);
            key.SetValue("DisplayVersion", Version);
            key.SetValue("Publisher", "NotaireDZ");
            key.SetValue("DisplayIcon", Path.Combine(target, "NotaireDZ.ico"));
            key.SetValue("InstallLocation", target);
            key.SetValue("UninstallString", "\"" + Path.Combine(target, "uninstall.cmd") + "\"");
            key.SetValue("NoModify", 1, RegistryValueKind.DWord);
            key.SetValue("NoRepair", 1, RegistryValueKind.DWord);
        }
    }

    private static string Run(string file, string arguments, string workingDirectory)
    {
        var start = new ProcessStartInfo(file, arguments)
        {
            WorkingDirectory = workingDirectory,
            UseShellExecute = false,
            RedirectStandardOutput = true,
            RedirectStandardError = true,
            CreateNoWindow = true
        };
        using (Process process = Process.Start(start))
        {
            string output = process.StandardOutput.ReadToEnd() + process.StandardError.ReadToEnd();
            process.WaitForExit();
            return "exit=" + process.ExitCode + " " + output.Trim().Replace("\r\n", " | ");
        }
    }

    private static void Note(string line)
    {
        Log.AppendLine(DateTime.Now.ToString("yyyy-MM-dd HH:mm:ss") + "  " + line);
        Flush();
    }

    private static void Flush()
    {
        try
        {
            File.WriteAllText(Path.Combine(Path.GetTempPath(), "NotaireDZ-setup.log"), Log.ToString());
        }
        catch
        {
            // Diagnostics must never break the install itself.
        }
    }

    private static int Fail(string message, bool quiet)
    {
        if (!quiet)
        {
            MessageBox.Show(message, Title, MessageBoxButtons.OK, MessageBoxIcon.Error);
        }
        return 1;
    }
}
