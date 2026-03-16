using System;
using System.Runtime.InteropServices;

namespace UnmSfx
{
    [StructLayout(LayoutKind.Sequential)]
    public readonly struct SfxHandle : IEquatable<SfxHandle>
    {
        internal readonly byte Raw;

        public static readonly SfxHandle Invalid = new(0);

        internal SfxHandle(byte value)
        {
            Raw = value;
        }

        public bool IsValid => Raw != 0;

        public bool Equals(SfxHandle other)
        {
            return Raw == other.Raw;
        }

        public override bool Equals(object obj)
        {
            return obj is SfxHandle other && Equals(other);
        }

        public override int GetHashCode()
        {
            return Raw.GetHashCode();
        }

        public static bool operator ==(SfxHandle left, SfxHandle right)
        {
            return left.Equals(right);
        }

        public static bool operator !=(SfxHandle left, SfxHandle right)
        {
            return !left.Equals(right);
        }

        public override string ToString()
        {
            return IsValid ? $"SfxHandle({Raw})" : "InvalidHandle";
        }
    }
}
