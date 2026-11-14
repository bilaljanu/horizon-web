# Data formats

The files Horizon writes that other tools may read: benchmark reports,
settings, content and worlds. Each entry names its version field, its
documentation and its level ([stability.md](stability.md)).

## Overview

| Format | Where | Version | Level | Documented in |
| --- | --- | --- | --- | --- |
| Benchmark report | the benchmark's output and the local log | `"format": "horizon-benchmark-report"`, `"version": 1` | stable | `crates/horizon-bench` |
| Content mod package | `.hmod`: a zip with `horizon-mod.toml`, `assets/`, `data/` | the manifest's `api` | preview | `crates/horizon-mods` |
| Mods turned on | `horizon-mods.txt` | none | preview | [content.md](content.md) |
| Horizon settings | `horizon-graphics.txt`, `horizon-extras.txt` (`key:value`, as `options.txt`) | none: unknown keys fall back to defaults | preview | [graphics.md](../graphics.md), [extras.md](../extras.md#options-storage) |
| Worlds | the world folder | vanilla 26.3's (`DataVersion`) | vanilla's | [world-data.md](../world-data.md) |

Worlds are vanilla's format, so any tool that reads a 26.3 world reads a Horizon world; Horizon adds itself to
`level.dat`'s `ServerBrands` as vanilla's server does.

## Rules for every format

- A reader ignores fields it doesn't know, and refuses a version newer than it knows with a clear message.
- An optional field may be added in the same version; renaming, removing or changing the meaning of a field makes the
  next version, and Horizon's own readers keep reading the older ones for the deprecation window.
- Each versioned format has a sample file of every version its reader accepts, read by a test.

## Not there yet

- Versions for the settings files.
- Sample files and reader tests for every versioned format.
- A generated schema for the benchmark report from its Rust types, checked by a test.

## Changelog

- Benchmark report version 1.
- Content mods can declare mob effects in `data/<id>/horizon/mob_effect/<name>.json` (`category`, `color`).
