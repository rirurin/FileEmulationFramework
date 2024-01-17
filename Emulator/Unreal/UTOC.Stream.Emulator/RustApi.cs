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
        public static extern void PrintEmulatedFile(string file_path);
    }
}
