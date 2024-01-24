using FileEmulationFramework.Interfaces;
using FileEmulationFramework.Lib.Utilities;
using System;
using System.Collections.Concurrent;
using System.Collections.Generic;
using System.Linq;
using System.Runtime.InteropServices;
using System.Text;
using System.Threading.Tasks;

namespace UTOC.Stream.Emulator
{
    public class UtocEmulator : IEmulator
    {
        public static readonly string UtocExtension = ".utoc";
        public bool DumpFiles { get; set; }
        public Logger _logger { get; init; }
        private readonly ConcurrentDictionary<string, System.IO.Stream?> _pathToStream = new(StringComparer.OrdinalIgnoreCase);

        public UtocEmulator(Logger logger) { _logger = logger; }

        public bool TryCreateFile(IntPtr handle, string filepath, string route, out IEmulatedFile emulated)
        {
            // Check if we've already made a custom UTOC
            emulated = null!;
            if (_pathToStream.TryGetValue(filepath, out var stream))
            {
                //_logger.Debug($"TryCreateFile: {filepath} already exists");
                if (stream == null) return false; // Avoid recursion into the same file
                return false;
            }
            // Check extension
            if (!filepath.EndsWith(UtocExtension, StringComparison.OrdinalIgnoreCase)) return false;

            // Check that the target file isn't the game's UTOC
            // We're interested in creating a patch UTOC

            //_logger.Debug($"TryCreateFile: Create a custom UTOC for {filepath}");

            if (!TryCreateEmulatedFile(handle, filepath, filepath, filepath, ref emulated!, out _)) return false;
            //_logger.Debug($"TryCreateFile: Successfully created a custom UTOC: {filepath}");
            return true;
        }

        /// <summary>
        /// Tries to create an emulated file from a given file handle.
        /// </summary>
        /// <param name="handle">Handle of the file where the data is sourced from.</param>
        /// <param name="srcDataPath">Path of the file where the handle refers to.</param>
        /// <param name="outputPath">Path where the emulated file is stored.</param>
        /// <param name="route">The route of the emulated file, for builder to pick up.</param>
        /// <param name="emulated">The emulated file.</param>
        /// <param name="stream">The created stream under the hood.</param>
        /// <returns>True if an emulated file could be created, false otherwise</returns>
        public bool TryCreateEmulatedFile(IntPtr handle, string srcDataPath, string outputPath, string route, ref IEmulatedFile? emulated, out System.IO.Stream? stream)
        {
            // Check if there's a known route for this file, put this before actual file check because I/O
            // Check file type
            // Make the table of contents (UTOC) and partition (UCAS)
            var result = RustApi.BuildTableOfContents(handle, srcDataPath, outputPath, route);
            stream = null;
            return result;
        }

        public void OnModLoading(string dir_path) => RustApi.AddFromFolders(dir_path);
    }
}
