# NOTICE

## opensim-rust

Copyright (c) [Copyright holders TBD]

Licensed under the GNU General Public License v3.0 or later (GPL-3.0-or-later).

## Aurora Viewer crates

This project reuses the following Aurora Viewer crates as git dependencies pinned
to commit `3d523b14` from https://github.com/Aurora-Viewer/Aurora-Viewer:

- `aurora-llsd` — LLSD serialization (binary, XML, notation)
- `aurora-msg` — Second Life message codec (483 messages from `message_template.msg`)
- `aurora-prim` — Primitive geometry, texture entries, extra params
- `aurora-net` — Network session types (re-exported through wrapper crates)
- `aurora-assets` — Mesh/physics import, including V-HACD for convex decomposition

The Aurora Viewer crates are licensed under GPL-3.0-or-later.

## Derived from Second Life viewer / Firestorm

`aurora-msg` derives `message_template.msg` from the Second Life viewer / Firestorm
code base. That source material is licensed under the LGPL 2.1 and is distributed
here within the GPL-3.0-or-later licensed project and crates.

## V-HACD

`aurora-assets` bundles V-HACD 4.1, licensed under the BSD 3-Clause License.

BSD 3-Clause License

Copyright (c) 2011, Khaled Mamou (kmamou at gmail dot com)
All rights reserved.

Redistribution and use in source and binary forms, with or without modification, are permitted provided that the following conditions are met:

1. Redistributions of source code must retain the above copyright notice, this list of conditions and the following disclaimer.

2. Redistributions in binary form must reproduce the above copyright notice, this list of conditions and the following disclaimer in the documentation and/or other materials provided with the distribution.

3. Neither the name of the copyright holder nor the names of its contributors may be used to endorse or promote products derived from this software without specific prior written permission.

THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS" AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
