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

        [DllImport("fileemu_utoc_stream_emulator")]
        public static extern void AddFromFolders(string mod_path);

        [DllImport("fileemu_utoc_stream_emulator")]
        public static extern IntPtr BuildTableOfContents(IntPtr handle, string srcDataPath, string outputPath, string route, ref long length);
    }
}
