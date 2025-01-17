using System.Runtime.InteropServices;
#pragma warning disable CS0649

namespace PAK.Stream.Emulator.Pak;

public struct V1FileEntry : IEntry
{
	private unsafe fixed byte _byteFileName[252];

	public int Length { get; }

    public string FileName => GetFileName();

    public unsafe string GetFileName()
	{
		fixed (byte* ptr = _byteFileName)
		{
			return Marshal.PtrToStringAnsi((nint)ptr)!;
		}
	}

    public void Dispose()
    {
    }
}
