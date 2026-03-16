# com.xlin.unmsfx

`com.xlin.unmsfx` is the Unity UPM package for the UnmSfx native runtime.

## Contents

- `Runtime/`: C# wrapper code and native binaries.
- `LICENSE.md`: Apache License 2.0 for the package's original source code.
- `THIRD-PARTY-NOTICES.txt`: Third-party notices copied from the repository
  root so the package remains compliant when distributed on its own.

## Install

Copy this folder into a Unity project's `Packages/` directory, or reference it
as a local UPM package from `manifest.json`.

## Basic Usage

```csharp
using UnmSfx;
using UnityEngine;

public sealed class UnmSfxExample : MonoBehaviour
{
    [SerializeField] private AudioClip clip;

    private void Start()
    {
        var handles = UnmSfxManager.Instance.LoadFromAudioClips(new[] { clip });
        if (handles.Length > 0)
        {
            UnmSfxManager.Instance.Play(handles[0]);
        }
    }
}
```

## Rebuild Native Binaries

Build the native runtime from `../unm-sfx/`, then replace the files under
`Runtime/bin/` with the outputs for each supported platform.
