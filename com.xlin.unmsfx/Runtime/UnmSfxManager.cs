using System;
using System.Collections.Generic;
using System.IO;
using UnityEngine;

namespace UnmSfx
{
    [DefaultExecutionOrder(-10000)]
    public sealed class UnmSfxManager : MonoBehaviour
    {
        private const int MaxSoundCount = 14;

        public static UnmSfxManager Ins
        {
            get
            {
                EnsureInstance();
                return _instance;
            }
        }

        private static UnmSfxManager _instance;
        private static bool _isInitialized;
        private static bool _isQuitting;
        private readonly int[] _framePlayCounts = new int[MaxSoundCount];

        private static void EnsureInstance()
        {
            if (_isQuitting || _instance != null)
            {
                return;
            }

            var host = new GameObject("UnmSfxManager");
            DontDestroyOnLoad(host);
            _instance = host.AddComponent<UnmSfxManager>();
        }

        private void Awake()
        {
            if (_instance != null && _instance != this)
            {
                Destroy(gameObject);
                return;
            }

            _instance = this;
            DontDestroyOnLoad(gameObject);

            if (!_isInitialized)
            {
                UnmSfxBridge.Init();
                _isInitialized = true;
            }
        }

        private void Update()
        {
            if (_isInitialized)
            {
                UnmSfxBridge.Tick();
            }
        }

        private void OnApplicationQuit()
        {
            _isQuitting = true;

            if (_isInitialized)
            {
                FlushFrameAlignedRequests();
                UnmSfxBridge.Shutdown();
                _isInitialized = false;
            }
        }

        private void OnDestroy()
        {
            if (_instance == this)
            {
                _instance = null;

                if (!_isQuitting && _isInitialized)
                {
                    FlushFrameAlignedRequests();
                    UnmSfxBridge.Shutdown();
                    _isInitialized = false;
                }
            }
        }

        private void LateUpdate()
        {
            if (!_isInitialized)
            {
                return;
            }

            FlushFrameAlignedRequests();
        }

        public SfxHandle[] LoadFromBytes(IReadOnlyList<byte[]> clipDataList)
        {
            if (!_isInitialized)
            {
                Debug.LogWarning("[UNM SFX] Manager is not initialized.");
                return Array.Empty<SfxHandle>();
            }

            return UnmSfxBridge.LoadSounds(clipDataList);
        }

        public SfxHandle[] LoadFromAudioClips(IReadOnlyList<AudioClip> clips)
        {
            if (!_isInitialized)
            {
                Debug.LogWarning("[UNM SFX] Manager is not initialized.");
                return Array.Empty<SfxHandle>();
            }

            if (clips == null || clips.Count == 0)
            {
                return Array.Empty<SfxHandle>();
            }

            return UnmSfxBridge.LoadAudioClips(clips);
        }

        public SfxHandle[] LoadFromFiles(IReadOnlyList<string> paths)
        {
            if (!_isInitialized)
            {
                Debug.LogWarning("[UNM SFX] Manager is not initialized.");
                return Array.Empty<SfxHandle>();
            }

            if (paths == null || paths.Count == 0)
            {
                return Array.Empty<SfxHandle>();
            }

            byte[][] clipData = new byte[paths.Count][];
            for (int i = 0; i < paths.Count; i++)
            {
                string path = paths[i];
                if (string.IsNullOrWhiteSpace(path) || !File.Exists(path))
                {
                    Debug.LogWarning($"[UNM SFX] File not found: {path}");
                    return Array.Empty<SfxHandle>();
                }

                clipData[i] = File.ReadAllBytes(path);
            }

            return LoadFromBytes(clipData);
        }

        public void Play(SfxHandle handle)
        {
            if (!_isInitialized)
            {
                Debug.LogWarning("[UNM SFX] Manager is not initialized.");
                return;
            }

            if (!handle.IsValid)
            {
                Debug.LogWarning("[UNM SFX] Attempted to play an invalid SfxHandle.");
                return;
            }

            UnmSfxBridge.Play(handle);
        }

        public void PlayFrameAligned(SfxHandle handle, int count = 1)
        {
            if (!_isInitialized)
            {
                Debug.LogWarning("[UNM SFX] Manager is not initialized.");
                return;
            }

            if (!handle.IsValid)
            {
                Debug.LogWarning("[UNM SFX] Attempted to queue an invalid SfxHandle.");
                return;
            }

            if (count <= 0)
            {
                return;
            }

            int index = handle.Raw - 1;
            if (index < 0 || index >= MaxSoundCount)
            {
                Debug.LogWarning($"[UNM SFX] Handle out of range: {handle}.");
                return;
            }

            _framePlayCounts[index] += count;
        }

        private void FlushFrameAlignedRequests()
        {
            for (int i = 0; i < MaxSoundCount; i++)
            {
                int remaining = _framePlayCounts[i];
                if (remaining <= 0)
                {
                    continue;
                }

                var handle = new SfxHandle((byte)(i + 1));
                while (remaining > 0)
                {
                    ushort chunk = (ushort)Math.Min(remaining, ushort.MaxValue);
                    int result = UnmSfxBridge.SubmitFramePlayCount(handle, chunk);
                    if (result != 0)
                    {
                        Debug.LogWarning(
                            $"[UNM SFX] submit_frame_play_count failed for {handle} (code {result})."
                        );
                        break;
                    }

                    remaining -= chunk;
                }

                _framePlayCounts[i] = 0;
            }
        }
    }
}
