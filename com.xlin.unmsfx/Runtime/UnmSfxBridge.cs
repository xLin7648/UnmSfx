using System;
using System.Collections.Generic;
using System.IO;
using System.Reflection;
using System.Runtime.InteropServices;
using System.Threading;
using UnityEngine;
#if UNITY_EDITOR
using UnityEditor;
using UnityEditor.PackageManager;
#endif

namespace UnmSfx
{
    internal static class UnmSfxBridge
    {
#if UNITY_IOS && !UNITY_EDITOR
        private const string LibName = "__Internal";
#elif UNITY_EDITOR_WIN
        private const string WindowsLibFileName = "unm_sfx.dll";
        private const string WindowsTempDirectoryName = "UnmSfxNative";

        [UnmanagedFunctionPointer(CallingConvention.Cdecl)]
        private delegate void InitDelegate();

        [UnmanagedFunctionPointer(CallingConvention.Cdecl)]
        private delegate void TickDelegate();

        [UnmanagedFunctionPointer(CallingConvention.Cdecl)]
        private delegate void ShutdownDelegate();

        [UnmanagedFunctionPointer(CallingConvention.Cdecl)]
        private delegate void PlayDelegate(byte handle);

        [UnmanagedFunctionPointer(CallingConvention.Cdecl)]
        private delegate int SubmitFramePlayCountDelegate(byte handle, ushort count);

        [UnmanagedFunctionPointer(CallingConvention.Cdecl)]
        private delegate int LoadSoundDelegate(
            IntPtr[] dataPtrs,
            UIntPtr[] dataLens,
            UIntPtr count,
            byte[] outHandles
        );

        [UnmanagedFunctionPointer(CallingConvention.Cdecl)]
        private delegate int LoadPcmF32Delegate(
            IntPtr[] dataPtrs,
            uint[] frameCounts,
            uint[] channelCounts,
            uint[] sampleRates,
            UIntPtr count,
            byte[] outHandles
        );

        private sealed class NativeApi
        {
            public IntPtr ModuleHandle;
            public string LoadedPath;
            public InitDelegate Init;
            public TickDelegate Tick;
            public ShutdownDelegate Shutdown;
            public PlayDelegate Play;
            public SubmitFramePlayCountDelegate SubmitFramePlayCount;
            public LoadSoundDelegate LoadSound;
            public LoadPcmF32Delegate LoadPcmF32;
        }

        [DllImport("kernel32", CharSet = CharSet.Unicode, SetLastError = true)]
        private static extern IntPtr LoadLibraryW(string fileName);

        [DllImport("kernel32", CharSet = CharSet.Ansi, SetLastError = true)]
        private static extern IntPtr GetProcAddress(IntPtr module, string procName);

        [DllImport("kernel32", SetLastError = true)]
        [return: MarshalAs(UnmanagedType.Bool)]
        private static extern bool FreeLibrary(IntPtr module);

        private static NativeApi s_api;
#else
        private const string LibName = "unm_sfx";
#endif

#if !UNITY_EDITOR_WIN
        [DllImport(LibName, CallingConvention = CallingConvention.Cdecl)]
        private static extern void unm_sfx_init();

        [DllImport(LibName, CallingConvention = CallingConvention.Cdecl)]
        private static extern void unm_sfx_tick();

        [DllImport(LibName, CallingConvention = CallingConvention.Cdecl)]
        private static extern void unm_sfx_shutdown();

        [DllImport(LibName, CallingConvention = CallingConvention.Cdecl)]
        private static extern void unm_sfx_play(byte handle);

        [DllImport(LibName, CallingConvention = CallingConvention.Cdecl)]
        private static extern int unm_sfx_submit_frame_play_count(byte handle, ushort count);

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
#endif

#if UNITY_EDITOR_WIN
        static UnmSfxBridge()
        {
            AssemblyReloadEvents.beforeAssemblyReload += ShutdownBeforeReload;
            EditorApplication.quitting += ShutdownBeforeReload;
        }
#endif

#if UNITY_EDITOR_WIN
        private static NativeApi GetApiOrNull()
        {
            return s_api;
        }

        private static NativeApi EnsureApi()
        {
            if (s_api != null)
            {
                return s_api;
            }

            string sourcePath = ResolveWindowsSourcePath();
            if (!File.Exists(sourcePath))
            {
                throw new DllNotFoundException($"[UNM SFX] Native library not found at '{sourcePath}'.");
            }

            string loadedPath = CreateLoadedCopy(sourcePath);
            IntPtr module = LoadLibraryW(loadedPath);
            if (module == IntPtr.Zero)
            {
                int error = Marshal.GetLastWin32Error();
                TryDeleteFile(loadedPath);
                throw new DllNotFoundException(
                    $"[UNM SFX] Failed to load native library copy '{loadedPath}' (Win32 error {error})."
                );
            }

            try
            {
                s_api = new NativeApi
                {
                    ModuleHandle = module,
                    LoadedPath = loadedPath,
                    Init = LoadFunction<InitDelegate>(module, "unm_sfx_init"),
                    Tick = LoadFunction<TickDelegate>(module, "unm_sfx_tick"),
                    Shutdown = LoadFunction<ShutdownDelegate>(module, "unm_sfx_shutdown"),
                    Play = LoadFunction<PlayDelegate>(module, "unm_sfx_play"),
                    SubmitFramePlayCount = LoadFunction<SubmitFramePlayCountDelegate>(
                        module,
                        "unm_sfx_submit_frame_play_count"
                    ),
                    LoadSound = LoadFunction<LoadSoundDelegate>(module, "unm_sfx_load_sound"),
                    LoadPcmF32 = LoadFunction<LoadPcmF32Delegate>(module, "unm_sfx_load_pcm_f32"),
                };

                return s_api;
            }
            catch
            {
                FreeLibrary(module);
                TryDeleteFile(loadedPath);
                throw;
            }
        }

        private static T LoadFunction<T>(IntPtr module, string name) where T : Delegate
        {
            IntPtr address = GetProcAddress(module, name);
            if (address == IntPtr.Zero)
            {
                throw new EntryPointNotFoundException(
                    $"[UNM SFX] Entry point '{name}' was not found in '{WindowsLibFileName}'."
                );
            }

            return Marshal.GetDelegateForFunctionPointer<T>(address);
        }

        private static string ResolveWindowsSourcePath()
        {
            string archFolder = IntPtr.Size == 8 ? "x86_64" : "x86";

#if UNITY_EDITOR
            var packageInfo = UnityEditor.PackageManager.PackageInfo.FindForAssembly(Assembly.GetExecutingAssembly());
            if (packageInfo != null && !string.IsNullOrEmpty(packageInfo.resolvedPath))
            {
                return Path.Combine(packageInfo.resolvedPath, "Runtime", "bin", archFolder, WindowsLibFileName);
            }

            string projectRoot = Directory.GetParent(Application.dataPath)?.FullName ?? string.Empty;
            if (!string.IsNullOrEmpty(projectRoot))
            {
                return Path.Combine(
                    projectRoot,
                    "Packages",
                    "com.xlin.unmsfx",
                    "Runtime",
                    "bin",
                    archFolder,
                    WindowsLibFileName
                );
            }
#endif

            return Path.Combine(Application.dataPath, "Plugins", archFolder, WindowsLibFileName);
        }

        private static string CreateLoadedCopy(string sourcePath)
        {
            string tempDirectory = Path.Combine(Path.GetTempPath(), WindowsTempDirectoryName, "Windows");
            Directory.CreateDirectory(tempDirectory);
            CleanupStaleCopies(tempDirectory);

            string loadedPath = Path.Combine(tempDirectory, $"unm_sfx_{Guid.NewGuid():N}.dll");
            File.Copy(sourcePath, loadedPath, false);
            return loadedPath;
        }

        private static void CleanupStaleCopies(string directory)
        {
            foreach (string path in Directory.GetFiles(directory, "unm_sfx_*.dll"))
            {
                try
                {
                    var age = DateTime.UtcNow - File.GetLastWriteTimeUtc(path);
                    if (age.TotalHours >= 1.0)
                    {
                        File.Delete(path);
                    }
                }
                catch
                {
                }
            }
        }

        private static void UnloadApi()
        {
            NativeApi api = s_api;
            s_api = null;
            if (api == null)
            {
                return;
            }

            if (api.ModuleHandle != IntPtr.Zero && !FreeLibrary(api.ModuleHandle))
            {
                Debug.LogWarning(
                    $"[UNM SFX] FreeLibrary failed for '{api.LoadedPath}' (Win32 error {Marshal.GetLastWin32Error()})."
                );
            }

            TryDeleteFile(api.LoadedPath);
        }

        private static void TryDeleteFile(string path)
        {
            if (string.IsNullOrEmpty(path) || !File.Exists(path))
            {
                return;
            }

            for (int attempt = 0; attempt < 8; attempt++)
            {
                try
                {
                    File.Delete(path);
                    return;
                }
                catch (IOException)
                {
                }
                catch (UnauthorizedAccessException)
                {
                }

                Thread.Sleep(25);
            }
        }

#if UNITY_EDITOR_WIN
        private static void ShutdownBeforeReload()
        {
            Shutdown();
        }
#endif
#endif

        private static void NativeInit()
        {
#if UNITY_EDITOR_WIN
            EnsureApi().Init();
#else
            unm_sfx_init();
#endif
        }

        private static void NativeTick()
        {
#if UNITY_EDITOR_WIN
            NativeApi api = GetApiOrNull();
            if (api != null)
            {
                api.Tick();
            }
#else
            unm_sfx_tick();
#endif
        }

        private static void NativeShutdown()
        {
#if UNITY_EDITOR_WIN
            NativeApi api = GetApiOrNull();
            if (api == null)
            {
                return;
            }

            try
            {
                api.Shutdown();
            }
            finally
            {
                UnloadApi();
            }
#else
            unm_sfx_shutdown();
#endif
        }

        private static void NativePlay(byte handle)
        {
#if UNITY_EDITOR_WIN
            NativeApi api = GetApiOrNull();
            if (api != null)
            {
                api.Play(handle);
            }
#else
            unm_sfx_play(handle);
#endif
        }

        private static int NativeSubmitFramePlayCount(byte handle, ushort count)
        {
#if UNITY_EDITOR_WIN
            NativeApi api = GetApiOrNull();
            if (api == null)
            {
                return -1;
            }

            return api.SubmitFramePlayCount(handle, count);
#else
            return unm_sfx_submit_frame_play_count(handle, count);
#endif
        }

        private static int NativeLoadSound(
            IntPtr[] dataPtrs,
            UIntPtr[] dataLens,
            UIntPtr count,
            byte[] outHandles
        )
        {
#if UNITY_EDITOR_WIN
            NativeApi api = GetApiOrNull();
            if (api == null)
            {
                return -1;
            }

            return api.LoadSound(dataPtrs, dataLens, count, outHandles);
#else
            return unm_sfx_load_sound(dataPtrs, dataLens, count, outHandles);
#endif
        }

        private static int NativeLoadPcmF32(
            IntPtr[] dataPtrs,
            uint[] frameCounts,
            uint[] channelCounts,
            uint[] sampleRates,
            UIntPtr count,
            byte[] outHandles
        )
        {
#if UNITY_EDITOR_WIN
            NativeApi api = GetApiOrNull();
            if (api == null)
            {
                return -1;
            }

            return api.LoadPcmF32(dataPtrs, frameCounts, channelCounts, sampleRates, count, outHandles);
#else
            return unm_sfx_load_pcm_f32(dataPtrs, frameCounts, channelCounts, sampleRates, count, outHandles);
#endif
        }

        public static void Init()
        {
            NativeInit();
        }

        public static void Tick()
        {
            NativeTick();
        }

        public static void Shutdown()
        {
            NativeShutdown();
        }

        public static void Play(SfxHandle handle)
        {
            if (handle.IsValid)
            {
                NativePlay(handle.Raw);
            }
        }

        public static int SubmitFramePlayCount(SfxHandle handle, ushort count)
        {
            if (!handle.IsValid || count == 0)
            {
                return 0;
            }

            return NativeSubmitFramePlayCount(handle.Raw, count);
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

                int result = NativeLoadSound(ptrs, lens, (UIntPtr)count, outHandles);
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

                int result = NativeLoadPcmF32(
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
