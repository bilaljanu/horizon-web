#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
: "${JAVA_HOME:?Set JAVA_HOME to a JDK 21 or newer}"
"$JAVA_HOME/bin/javac" --release 21 -d tests/data testdata/src/t/*.java
cp tests/data/t/Test.class tests/data/Test.class
cp tests/data/t/Mark.class tests/data/Mark.class
build=$(mktemp -d)
trap 'rm -rf "$build"' EXIT
find testdata/api testdata/mods -name '*.java' > "$build/sources"
"$JAVA_HOME/bin/javac" --release 21 -d "$build/classes" @"$build/sources"
python3 testdata/assets.py "$build/assets"
"$JAVA_HOME/bin/jar" --create --date=2026-01-01T00:00:00Z --file tests/data/fabric-probe.jar -C "$build/classes" fixture/FabricProbe.class -C "$build/classes" fixture/Registrations.class -C testdata fabric.mod.json -C "$build/assets/fabricprobe" assets -C "$build/assets/fabricprobe" data
"$JAVA_HOME/bin/jar" --create --date=2026-01-01T00:00:00Z --file tests/data/neoforge-probe.jar -C "$build/classes" fixture/NeoForgeProbe.class -C "$build/classes" fixture/Registrations.class -C testdata/neoforge META-INF/neoforge.mods.toml -C "$build/assets/neoprobe" assets -C "$build/assets/neoprobe" data
