using System;
using System.Collections.Generic;
using System.IO;
using UnityEngine;

namespace UnmSfx
{
    [DefaultExecutionOrder(-10000)]
    public sealed class UnmSfxManager : MonoBehaviour
    {
        public static UnmSfxManager Instance
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
                    UnmSfxBridge.Shutdown();
                    _isInitialized = false;
                }
            }
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
    }
}
