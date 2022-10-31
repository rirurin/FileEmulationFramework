﻿using System.Runtime.CompilerServices;
using System.Runtime.InteropServices;
using AwbLib.Structs;
using FileEmulationFramework.Lib.Utilities;
using Microsoft.Win32.SafeHandles;

namespace AwbLib.Utilities;

/// <summary>
/// Checks if the file is an AWB file.
/// </summary>
public static class AwbChecker
{
    /// <summary>
    /// Checks if a file with a given handle is an AFS file.
    /// </summary>
    /// <param name="handle">The file handle to use.</param>
    public static bool IsAwbFile(IntPtr handle)
    {
        var fileStream = new FileStream(new SafeFileHandle(handle, false), FileAccess.Read);
        var pos = fileStream.Position;

        try
        {
            return Read<int>(fileStream) == Afs2Header.ExpectedMagic; // 'AFS2'
        }
        finally
        {
            fileStream.Dispose();
            Native.SetFilePointerEx(handle, pos, IntPtr.Zero, 0);
        }
    }
    
    [MethodImpl(MethodImplOptions.AggressiveInlining)]
    private static T Read<T>(this System.IO.Stream stream) where T : unmanaged
    {
        Span<T> stackSpace = stackalloc T[1];
        stream.TryRead(MemoryMarshal.Cast<T, byte>(stackSpace), out _);
        return stackSpace[0];
    }
}