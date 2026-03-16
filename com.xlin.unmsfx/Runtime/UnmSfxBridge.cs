using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using UnityEngine;

namespace UnmSfx
{
    internal static class UnmSfxBridge
    {
#if UNITY_IOS && !UNITY_EDITOR
    private const string LibName = "__Internal";
#else
        private const string LibName = "unm_sfx";
#endif

        [DllImport(LibName, CallingConvention = CallingConvention.Cdecl)]
        private static extern void unm_sfx_init();

        [DllImport(LibName, CallingConvention = CallingConvention.Cdecl)]
        private static extern void unm_sfx_tick();

        [DllImport(LibName, CallingConvention = CallingConvention.Cdecl)]
        private static extern void unm_sfx_shutdown();

        [DllImport(LibName, CallingConvention = CallingConvention.Cdecl)]
        private static extern void unm_sfx_play(byte handle);

        [DllImport(LibName, CallingConvention = CallingConvention.Cdecl)]
        private static extern int unm_sfx_load_sound(
            IntPtr[] dataPtrs,
            UIntPtr[] dataLens,
            UIntPtr count,
            byte[] outHandles
        );

        [DllImport(LibName, CallingConvention = CallingConvention.Cdecl)]
        private static extern int unm_sfx_load_pcm_f32(
            IntPtr[] dataPtrs,
            uint[] frameCounts,
            uint[] channelCounts,
            uint[] sampleRates,
            UIntPtr count,
            byte[] outHandles
        );

        public static void Init()
        {
            unm_sfx_init();
        }

        public static void Tick()
        {
            unm_sfx_tick();
        }

        public static void Shutdown()
        {
            unm_sfx_shutdown();
        }

        public static void Play(SfxHandle handle)
        {
            if (handle.IsValid)
            {
                unm_sfx_play(handle.Raw);
            }
        }

        public static SfxHandle[] LoadSounds(IReadOnlyList<byte[]> clipDataList)
        {
            if (clipDataList == null || clipDataList.Count == 0)
            {
                return Array.Empty<SfxHandle>();
            }

            int count = clipDataList.Count;
            IntPtr[] ptrs = new IntPtr[count];
            UIntPtr[] lens = new UIntPtr[count];
            byte[] outHandles = new byte[count];
            GCHandle[] pinnedHandles = new GCHandle[count];

            try
            {
                for (int i = 0; i < count; i++)
                {
                    byte[] clipData = clipDataList[i];
                    if (clipData == null || clipData.Length == 0)
                    {
                        Debug.LogError($"[UNM SFX] Clip data at index {i} is null or empty.");
                        return Array.Empty<SfxHandle>();
                    }

                    pinnedHandles[i] = GCHandle.Alloc(clipData, GCHandleType.Pinned);
                    ptrs[i] = pinnedHandles[i].AddrOfPinnedObject();
                    lens[i] = (UIntPtr)clipData.Length;
                }

                int result = unm_sfx_load_sound(ptrs, lens, (UIntPtr)count, outHandles);
                if (result != 0)
                {
                    Debug.LogError($"[UNM SFX] LoadSounds failed with error code {result}.");
                    return Array.Empty<SfxHandle>();
                }

                SfxHandle[] handles = new SfxHandle[count];
                for (int i = 0; i < count; i++)
                {
                    handles[i] = new SfxHandle(outHandles[i]);
                }

                return handles;
            }
            finally
            {
                for (int i = 0; i < count; i++)
                {
                    if (pinnedHandles[i].IsAllocated)
                    {
                        pinnedHandles[i].Free();
                    }
                }
            }
        }

        public static SfxHandle[] LoadAudioClips(IReadOnlyList<AudioClip> clips)
        {
            if (clips == null || clips.Count == 0)
            {
                return Array.Empty<SfxHandle>();
            }

            int count = clips.Count;
            IntPtr[] ptrs = new IntPtr[count];
            uint[] frameCounts = new uint[count];
            uint[] channelCounts = new uint[count];
            uint[] sampleRates = new uint[count];
            byte[] outHandles = new byte[count];
            GCHandle[] pinnedHandles = new GCHandle[count];

            try
            {
                for (int i = 0; i < count; i++)
                {
                    AudioClip clip = clips[i];
                    if (clip == null)
                    {
                        Debug.LogError($"[UNM SFX] AudioClip at index {i} is null.");
                        return Array.Empty<SfxHandle>();
                    }

                    if (clip.samples <= 0 || clip.channels <= 0 || clip.frequency <= 0)
                    {
                        Debug.LogError($"[UNM SFX] AudioClip '{clip.name}' has invalid metadata.");
                        return Array.Empty<SfxHandle>();
                    }

                    long sampleCount = (long)clip.samples * clip.channels;
                    if (sampleCount > int.MaxValue)
                    {
                        Debug.LogError($"[UNM SFX] AudioClip '{clip.name}' is too large to load.");
                        return Array.Empty<SfxHandle>();
                    }

                    float[] pcmData = new float[(int)sampleCount];
                    if (!clip.GetData(pcmData, 0))
                    {
                        Debug.LogError(
                            $"[UNM SFX] Failed to read AudioClip '{clip.name}'. " +
                            "Make sure Unity has loaded the clip data and that sample access is allowed."
                        );
                        return Array.Empty<SfxHandle>();
                    }

                    pinnedHandles[i] = GCHandle.Alloc(pcmData, GCHandleType.Pinned);
                    ptrs[i] = pinnedHandles[i].AddrOfPinnedObject();
                    frameCounts[i] = (uint)clip.samples;
                    channelCounts[i] = (uint)clip.channels;
                    sampleRates[i] = (uint)clip.frequency;
                }

                int result = unm_sfx_load_pcm_f32(
                    ptrs,
                    frameCounts,
                    channelCounts,
                    sampleRates,
                    (UIntPtr)count,
                    outHandles
                );
                if (result != 0)
                {
                    Debug.LogError($"[UNM SFX] LoadAudioClips failed with error code {result}.");
                    return Array.Empty<SfxHandle>();
                }

                SfxHandle[] handles = new SfxHandle[count];
                for (int i = 0; i < count; i++)
                {
                    handles[i] = new SfxHandle(outHandles[i]);
                }

                return handles;
            }
            finally
            {
                for (int i = 0; i < count; i++)
                {
                    if (pinnedHandles[i].IsAllocated)
                    {
                        pinnedHandles[i].Free();
                    }
                }
            }
        }
    }
}
