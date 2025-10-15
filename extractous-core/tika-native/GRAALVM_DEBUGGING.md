# GraalVM Native-Image Debugging Guide

This guide documents the methodology for fixing GraalVM reflection metadata issues in the extractous project.

## When to Use This Approach

Use the GraalVM tracing agent when you encounter errors like:

- `TIKA-198: Illegal IOException`
- `NoClassDefFoundError` at runtime (but class exists in JVM)
- `ClassNotFoundException` in native-image but not in JVM
- Parsing failures for specific file types that work with standard Tika
- Any error that works in JVM mode but fails in GraalVM native-image

## Symptoms of Missing Reflection Metadata

1. **File works with Apache Tika directly** (JVM mode)
2. **File fails with extractous** (native-image mode)
3. **Error is vague** (like TIKA-198) without specific class information
4. **Happens with certain file types** (especially those with embedded objects, images, or complex structures)

## The Proper Methodology

### Overview

The GraalVM tracing agent captures **all** reflective calls, resource accesses, JNI calls, and proxy usage during a JVM test run. This eliminates guesswork about which classes are needed.

### Step-by-Step Process

#### 1. Create a Java Test Harness

Create a simple Java class that reproduces the issue in JVM mode:

```bash
# Location: tika-native/src/main/java/ai/yobix/AgentTest.java
```

```java
package ai.yobix;

import org.apache.tika.Tika;
import java.io.File;

public class AgentTest {
    public static void main(String[] args) throws Exception {
        System.out.println("Starting Tika parse with GraalVM agent...");
        Tika tika = new Tika();

        // Parse the problematic file
        String filePath = "../../test_files/your-problem-file.docx";
        File file = new File(filePath);

        if (!file.exists()) {
            System.err.println("File not found: " + file.getAbsolutePath());
            System.exit(1);
        }

        System.out.println("Parsing file: " + file.getAbsolutePath());
        String content = tika.parseToString(file);

        System.out.println("Successfully extracted content!");
        System.out.println("Content length: " + content.length());
        System.out.println("First 200 chars: " + content.substring(0, Math.min(200, content.length())));
        System.out.println("Agent test completed successfully.");
    }
}
```

#### 2. Add Gradle Task for Classpath

Add this to `tika-native/build.gradle`:

```gradle
task printClasspath {
    doLast {
        println configurations.runtimeClasspath.asPath
    }
}
```

#### 3. Compile the Test Class

```bash
cd extractous-core/tika-native
export JAVA_HOME="/path/to/graalvm-jdk"
./gradlew --no-daemon compileJava
```

#### 4. Run with GraalVM Tracing Agent

```bash
# Create output directory for agent config
rm -rf agent-config
mkdir -p agent-config

# Set Java home to GraalVM
export JAVA_HOME="/path/to/graalvm-jdk"

# Get the classpath
CLASSPATH="build/classes/java/main:$(./gradlew --no-daemon --quiet printClasspath)"

# Run with the native-image-agent
$JAVA_HOME/bin/java \
  -agentlib:native-image-agent=config-output-dir=agent-config \
  -cp "$CLASSPATH" \
  ai.yobix.AgentTest
```

**Expected output:**
```
Starting Tika parse with GraalVM agent...
Parsing file: /path/to/your-problem-file.docx
Successfully extracted content!
Content length: 3856
First 200 chars: [content preview]
Agent test completed successfully.
```

This will generate:
- `agent-config/reachability-metadata.json` - Reflection metadata
- `agent-config/resource-config.json` - Resource bundles (if any)
- `agent-config/proxy-config.json` - Dynamic proxy metadata (if any)
- `agent-config/jni-config.json` - JNI metadata (if any)

#### 5. Create Metadata Merge Script

Create `merge_metadata.py` in the project root:

```python
#!/usr/bin/env python3
"""
Merge GraalVM agent-generated metadata with existing metadata.
"""

import json
import sys

def merge_metadata(existing_file, agent_file, output_file):
    """Merge agent-generated metadata into existing metadata."""
    print(f"Loading existing metadata from {existing_file}...")
    with open(existing_file, 'r') as f:
        existing = json.load(f)

    print(f"Loading agent metadata from {agent_file}...")
    with open(agent_file, 'r') as f:
        agent = json.load(f)

    # Get existing types to avoid duplicates
    existing_types = set()
    for entry in existing.get('reflection', []):
        if 'type' in entry:
            existing_types.add(entry['type'])

    # Add new entries from agent
    new_entries = []
    for entry in agent.get('reflection', []):
        if 'type' in entry and entry['type'] not in existing_types:
            new_entries.append(entry)
            existing_types.add(entry['type'])

    if new_entries:
        print(f"Adding {len(new_entries)} new reflection entries...")
        existing['reflection'].extend(new_entries)

        # Also merge resources if present
        if 'resources' in agent:
            if 'resources' not in existing:
                existing['resources'] = []

            # Handle both list and dict formats
            if isinstance(existing['resources'], list):
                existing_resources_list = existing['resources']
            else:
                existing_resources_list = existing['resources'].get('includes', [])

            existing_resources = set()
            for r in existing_resources_list:
                if 'glob' in r:
                    existing_resources.add(r['glob'])
                elif 'pattern' in r:
                    existing_resources.add(r['pattern'])

            # Handle both list and dict formats for agent resources
            if isinstance(agent['resources'], list):
                agent_resources_list = agent['resources']
            else:
                agent_resources_list = agent['resources'].get('includes', [])

            for r in agent_resources_list:
                resource_key = r.get('glob') or r.get('pattern')
                if resource_key and resource_key not in existing_resources:
                    existing_resources_list.append(r)
                    existing_resources.add(resource_key)

            print(f"Merged resources section")

        print(f"Writing merged metadata to {output_file}...")
        with open(output_file, 'w') as f:
            json.dump(existing, f, indent=4)

        print(f"✓ Successfully merged {len(new_entries)} new entries!")
        return True
    else:
        print("No new entries to add.")
        return False

if __name__ == "__main__":
    base_dir = "extractous-core/tika-native/src/main/resources/META-INF/ai.yobix/"

    for platform in ["tika-2.9.3-linux", "tika-2.9.3-macos", "tika-2.9.3-windows"]:
        existing_file = f"{base_dir}{platform}/reachability-metadata.json"
        agent_file = "extractous-core/tika-native/agent-config/reachability-metadata.json"

        print(f"\n{'='*60}")
        print(f"Processing {platform}...")
        print(f"{'='*60}")

        if merge_metadata(existing_file, agent_file, existing_file):
            print(f"✓ Updated {platform}")
        else:
            print(f"✓ No changes needed for {platform}")

    print("\n" + "="*60)
    print("✓ Metadata merge complete for all platforms!")
    print("="*60)
```

#### 6. Merge the Metadata

```bash
cd extractous-core/tika-native
python3 ../../merge_metadata.py
```

**Expected output:**
```
============================================================
Processing tika-2.9.3-linux...
============================================================
Loading existing metadata from .../tika-2.9.3-linux/reachability-metadata.json...
Loading agent metadata from agent-config/reachability-metadata.json...
Adding 5 new reflection entries...
Merged resources section
Writing merged metadata to .../tika-2.9.3-linux/reachability-metadata.json...
✓ Successfully merged 5 new entries!
✓ Updated tika-2.9.3-linux

[... similar for macOS and Windows ...]

============================================================
✓ Metadata merge complete for all platforms!
============================================================
```

#### 7. Force Rebuild of Native Library

```bash
cd extractous-core

# Remove cached native builds to force rebuild
rm -rf target/debug/build/extractous-*/out/tika-native
rm -rf target/debug/build/extractous-*/out/libs

# Touch metadata to trigger rebuild
touch tika-native/src/main/resources/META-INF/ai.yobix/tika-2.9.3-linux/reachability-metadata.json

# Build (this will take several minutes)
cargo build --lib
```

The build process will:
1. Copy updated tika-native sources to OUT_DIR
2. Download GraalVM if needed
3. Run Gradle nativeCompile (5-10 minutes)
4. Build the Rust library

#### 8. Test the Fix

```bash
# Run your specific test
cargo test test_your_issue_name

# Run full test suite
cargo test --lib
cargo test --test extract_to_string_tests
```

**Expected result:**
```
test test_issue_52_chinese_docx ... ok
test result: ok. 1 passed; 0 failed
```

#### 9. Clean Up and Commit

```bash
# Remove temporary files
rm -rf tika-native/agent-config
rm -f tika-native/src/main/java/ai/yobix/AgentTest.java
rm -f merge_metadata.py

# Stage changes
git add -A

# Commit
git commit -m "Fix issue #XX: [Description]

Root Cause:
[Explain the missing metadata issue]

Solution:
1. Created Java test harness using Apache Tika directly
2. Ran the test with GraalVM native-image-agent to capture all reflective calls
3. Merged the agent-generated metadata with existing configuration
4. Added reflection metadata for N previously missing classes

Changes:
- Added test case: tests/test_issue_XX.rs
- Added test file: test_files/issue-XX-file.ext
- Updated GraalVM reflection metadata for all platforms

Testing:
- Test case test_issue_XX_name now passes
- All existing unit tests continue to pass

Fixes #XX

🤖 Generated with [Claude Code](https://claude.com/claude-code)

Co-Authored-By: Claude <noreply@anthropic.com>
"
```

## Common Issues and Solutions

### Issue: Agent doesn't capture metadata

**Symptom:** `agent-config` directory is empty or has no reflection entries

**Solutions:**
- Ensure the file actually triggers the problematic code path
- Verify JAVA_HOME points to GraalVM (not regular JDK)
- Check that the test runs successfully before adding the agent
- Try with `-agentlib:native-image-agent=config-merge-dir=agent-config` to append to existing config

### Issue: Build doesn't rebuild native library

**Symptom:** Build completes in seconds, test still fails

**Solutions:**
```bash
# Force complete rebuild
cargo clean
rm -rf target/debug/build/extractous-*/out/tika-native
rm -rf target/debug/build/extractous-*/out/libs

# Touch metadata file to trigger change detection
touch tika-native/src/main/resources/META-INF/ai.yobix/tika-2.9.3-linux/reachability-metadata.json

# Rebuild
cargo build --lib
```

### Issue: Finding GraalVM path

**Symptom:** Don't know where GraalVM is installed

**Solutions:**
```bash
# If using extractous build system, it downloads GraalVM to:
find target/debug/build -name "graalvm-jdk" -type d

# Example path:
# /path/to/extractous-core/target/debug/build/extractous-HASH/out/graalvm-jdk/graalvm-community-openjdk-23.0.1+11.1
```

### Issue: Classpath issues

**Symptom:** `NoClassDefFoundError` when running AgentTest

**Solutions:**
```bash
# Verify gradle built successfully
./gradlew --no-daemon build

# Manually check classpath
./gradlew --no-daemon printClasspath

# Verify test class compiled
ls -la build/classes/java/main/ai/yobix/AgentTest.class
```

## Advanced: Inspecting Metadata

### View new classes added:

```bash
cd tika-native
python3 -c "
import json
with open('agent-config/reachability-metadata.json') as f:
    data = json.load(f)
    for entry in data.get('reflection', []):
        if 'type' in entry:
            print(entry['type'])
" | head -20
```

### Compare metadata before/after:

```bash
# Count reflection entries
echo "Before:"
cat src/main/resources/META-INF/ai.yobix/tika-2.9.3-linux/reachability-metadata.json | \
  python3 -c "import json,sys; print(len(json.load(sys.stdin)['reflection']))"

echo "Agent captured:"
cat agent-config/reachability-metadata.json | \
  python3 -c "import json,sys; print(len(json.load(sys.stdin)['reflection']))"
```

### Find specific class patterns:

```bash
# Find all POI classes in metadata
cat agent-config/reachability-metadata.json | \
  python3 -c "import json,sys;
data = json.load(sys.stdin)
for entry in data.get('reflection', []):
    if 'type' in entry and 'org.apache.poi' in entry['type']:
        print(entry['type'])" | sort
```

## Why This Approach Works

### Traditional Approach (Manual)
❌ **Guess** which classes might be needed
❌ **Trial and error** - add classes, rebuild, test, repeat
❌ **Time consuming** - each rebuild takes 5-10 minutes
❌ **Incomplete** - easy to miss transitive dependencies

### GraalVM Agent Approach (Automated)
✅ **Captures everything** - agent records all reflective calls
✅ **One iteration** - capture once, rebuild once
✅ **Complete** - includes all transitive dependencies
✅ **Reproducible** - same methodology for future issues

## Reference: Issue #52 Example

**Problem:** Chinese DOCX with 36 embedded OLE objects (math equations) failed with TIKA-198

**Traditional approach would have required:**
- Guessing 20+ POI/OOXML classes
- 5-10 rebuild cycles (50-100 minutes)
- Still might miss some classes

**Agent approach took:**
- 1 agent run (30 seconds)
- 1 rebuild (10 minutes)
- Complete fix with all 5 needed classes

**Files involved:**
- Test: `tests/test_issue_52.rs`
- Test file: `test_files/issue-52-chinese.docx`
- Commit: `28de320 Fix issue #52: TIKA-198 IOException with Chinese DOCX file`

## Quick Reference Command List

```bash
# 1. Create agent test class
cat > tika-native/src/main/java/ai/yobix/AgentTest.java << 'EOF'
[Java code here]
EOF

# 2. Compile
cd tika-native && ./gradlew --no-daemon compileJava

# 3. Run with agent
mkdir -p agent-config
JAVA_HOME="/path/to/graalvm" CLASSPATH="build/classes/java/main:$(./gradlew --no-daemon --quiet printClasspath)" \
  $JAVA_HOME/bin/java -agentlib:native-image-agent=config-output-dir=agent-config \
  -cp "$CLASSPATH" ai.yobix.AgentTest

# 4. Merge metadata
cd ../.. && python3 merge_metadata.py

# 5. Rebuild
cd extractous-core && \
  rm -rf target/debug/build/extractous-*/out/tika-native && \
  touch tika-native/src/main/resources/META-INF/ai.yobix/tika-2.9.3-linux/reachability-metadata.json && \
  cargo build --lib

# 6. Test
cargo test test_your_issue_name

# 7. Clean up
rm -rf tika-native/agent-config tika-native/src/main/java/ai/yobix/AgentTest.java ../merge_metadata.py

# 8. Commit
git add -A && git commit -m "Fix issue #XX: [description]"
```

## Additional Resources

- [GraalVM Native Image Metadata](https://www.graalvm.org/latest/reference-manual/native-image/metadata/)
- [Native Image Agent Guide](https://www.graalvm.org/latest/reference-manual/native-image/metadata/AutomaticMetadataCollection/)
- [Reflection in Native Image](https://www.graalvm.org/latest/reference-manual/native-image/dynamic-features/Reflection/)
