using System;
using System.Collections.Generic;
using System.Linq;
using System.Runtime.InteropServices;
using System.Text;
using System.Threading.Tasks;

namespace UTOC.Stream.Emulator
{
    public static class RustApi
    {

        [DllImport("fileemu_utoc_stream_emulator")] // Collect assets
        public static extern void AddFromFolders(string mod_id, string mod_path);

        [DllImport("fileemu_utoc_stream_emulator")] // Build UTOC
        public static extern IntPtr BuildTableOfContents(IntPtr handle, string srcDataPath, string outputPath, string route, ref long length);

        [DllImport("fileemu_utoc_stream_emulator")] // Build UCAS
        public static extern IntPtr BuildVirtualContainer();
    }
}
