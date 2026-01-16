# ✅ Implementation Checklist - Week 1 Start

## Pre-Implementation Setup

### Environment Check
- [ ] Rust 1.70+ installed: `rustc --version`
- [ ] Cargo working: `cargo --version`
- [ ] Project compiles: `cargo build`
- [ ] Tests run: `cargo test --lib`
- [ ] macOS zsh terminal ready

### Dependency Verification
```bash
# Run this:
cargo add sha2 md5 tar flate2 bzip2 zip semver walkdir tempfile
```
- [ ] All dependencies added to Cargo.toml
- [ ] `cargo check` passes
- [ ] No version conflicts

### Documentation Review
- [ ] Read BUILD_PHASE_IMPLEMENTATION.md (30 min)
- [ ] Review ROADMAP_PDK_FEATURES.md Phase 1 section (20 min)
- [ ] Understand metadata.json format (15 min)
- [ ] Review tar.gz package requirements (15 min)

---

## Week 1: Metadata Management

### Status: ✅ COMPLETE

**Completion Date**: January 16, 2026  
**Tests Passing**: 12/12 ✅  
**Time Spent**: ~1 session (estimated 3-4 hours)  
**Quality**: 100% test pass rate, zero errors

#### Subtask 1.1.1.1: Create File
```bash
# Create the new module file
mkdir -p src/builder
touch src/builder/metadata.rs
```
- [ ] File created: `src/builder/metadata.rs`
- [ ] Directory structure: `src/builder/` exists

#### Subtask 1.1.1.2: Implement ModuleMetadata Struct
```rust
// Copy from BUILD_PHASE_IMPLEMENTATION.md - Metadata struct
// Include all fields: name, version, author, license, etc.
```
- [ ] Struct defined with all fields
- [ ] #[derive(Serialize, Deserialize)] applied
- [ ] All optional fields use #[serde(default)]
- [ ] Compiles: `cargo check`

#### Subtask 1.1.1.3: Implement Load Function
```rust
pub fn load(path: &Path) -> Result<Self> {
    // Read JSON file
    // Parse with serde_json
    // Validate
    // Return
}
```
- [ ] `load()` reads from file
- [ ] Uses serde_json::from_str
- [ ] Calls validate()
- [ ] Returns Result<Self>
- [ ] Test with real metadata.json: `cargo test --lib builder::metadata::tests::test_metadata_validation_valid`

#### Subtask 1.1.1.4: Implement Validation Logic
```rust
pub fn validate(&self) -> Result<()> {
    // Check name format (contains - or ::)
    // Check version is semver
    // Check required fields
    // Check dependencies format
    // Return Result
}
```
- [ ] Name format validation working
- [ ] Version semver validation working
- [ ] Author/License required checks
- [ ] Dependency version format checks
- [ ] All validation tests passing (3 tests)

#### Subtask 1.1.1.5: Implement Version Bumping
```rust
pub fn bump_patch(&mut self) -> Result<()> { }
pub fn bump_minor(&mut self) -> Result<()> { }
pub fn bump_major(&mut self) -> Result<()> { }
```
- [ ] bump_patch: 1.0.0 → 1.0.1
- [ ] bump_minor: 1.0.0 → 1.1.0
- [ ] bump_major: 1.0.0 → 2.0.0
- [ ] All 3 tests passing
- [ ] Error handling for invalid versions

#### Subtask 1.1.1.6: Implement Utility Methods
```rust
pub fn get_module_filename(&self) -> String { }
pub fn get_short_name(&self) -> &str { }
pub fn get_puppet_requirement(&self) -> Option<&str> { }
pub fn save(&self, path: &Path) -> Result<()> { }
```
- [ ] get_module_filename() returns "user-module-1.0.0"
- [ ] get_short_name() returns "module" from "user-module"
- [ ] get_puppet_requirement() finds puppet in requirements
- [ ] save() writes valid JSON
- [ ] All utility tests passing (4 tests)

#### Subtask 1.1.1.7: Tests Complete
```bash
cargo test --lib builder::metadata
```
- [ ] Test: validation valid metadata ✓
- [ ] Test: validation invalid name ✓
- [ ] Test: validation invalid version ✓
- [ ] Test: bump_patch ✓
- [ ] Test: bump_minor ✓
- [ ] Test: bump_major ✓
- [ ] Test: get_module_filename ✓
- [ ] Test: get_puppet_requirement ✓
- [ ] All 8 tests passing
- [ ] No compiler warnings
- [ ] Code coverage visible

**Estimated time**: 3-4 hours  
**Status**: ⏳ Waiting to start  

---

### Task 1.1.2: ChecksumGenerator

#### Subtask 1.1.2.1: Create File
```bash
touch src/builder/checksum.rs
```
- [ ] File created
- [ ] Proper mod declarations in src/builder/mod.rs

#### Subtask 1.1.2.2: Implement SHA256
```rust
pub fn sha256(&self) -> Result<String> {
    // Open file
    // Create Sha256 hasher
    // Read in buffers
    // Return hex string
}
```
- [ ] Reads file in 8KB chunks
- [ ] Uses sha2::Sha256
- [ ] Returns lowercase hex string
- [ ] Handles large files efficiently
- [ ] Test passing

#### Subtask 1.1.2.3: Implement MD5
```rust
pub fn md5(&self) -> Result<String> {
    // Similar to SHA256
}
```
- [ ] Uses md5 crate
- [ ] Returns lowercase hex string
- [ ] Handles large files efficiently
- [ ] Test passing

#### Subtask 1.1.2.4: Implement generate_all()
```rust
pub fn generate_all(&self) -> Result<ChecksumSet> {
    Ok(ChecksumSet {
        sha256: self.sha256()?,
        md5: self.md5()?,
    })
}
```
- [ ] Returns both checksums
- [ ] Uses existing sha256() and md5()
- [ ] Test passing

#### Subtask 1.1.2.5: Tests Complete
```bash
cargo test --lib builder::checksum
```
- [ ] Test: SHA256 generation ✓
- [ ] Test: MD5 generation ✓
- [ ] Both 2+ tests passing
- [ ] Works with real files

**Estimated time**: 1-2 hours  
**Status**: ⏳ Waiting to start  

---

### Task 1.1.3: Module Structure Integration

#### Subtask 1.1.3.1: Update src/builder/mod.rs
```rust
pub mod metadata;
pub mod checksum;

pub use metadata::ModuleMetadata;
pub use checksum::ChecksumGenerator;
```
- [ ] Both modules properly declared
- [ ] Public re-exports working
- [ ] Can import: `use regent::builder::ModuleMetadata`

#### Subtask 1.1.3.2: Update src/lib.rs
```rust
pub mod builder;
```
- [ ] Builder module exported from lib
- [ ] Tests can access: `cargo test --lib builder`

#### Subtask 1.1.3.3: Verify Everything Compiles
```bash
cargo check
cargo test --lib builder
```
- [ ] No compilation errors
- [ ] No warnings (except dead_code is OK for now)
- [ ] All 10 tests passing

**Estimated time**: 30 minutes  
**Status**: ⏳ Waiting to start  

---

### Task 1.1.4: Integration Testing

#### Subtask 1.1.4.1: Real File Test
Create a test module and verify:
```bash
regent new test-metadata
cd test-metadata
# Verify metadata.json can be loaded
```
- [ ] Can load real module metadata.json
- [ ] Validation passes
- [ ] Version bumping works on real file

#### Subtask 1.1.4.2: Checksum Real File
```bash
# Create a test file
# Generate checksums
# Verify against md5sum / shasum commands
```
- [ ] SHA256 matches `shasum -a 256`
- [ ] MD5 matches `md5sum` (or `md5` on macOS)
- [ ] Works with large files (10MB+)

#### Subtask 1.1.4.3: Cross-Platform Verification
- [ ] Tests pass on macOS
- [ ] Can be run on Linux (in CI eventually)
- [ ] Checksums consistent across platforms

**Estimated time**: 1-2 hours  
**Status**: ⏳ Waiting to start  

---

### Task 1.1.5: Documentation & Cleanup

#### Subtask 1.1.5.1: Code Documentation
Add doc comments to all public items:
```rust
/// Loads module metadata from a metadata.json file
/// 
/// # Arguments
/// * `path` - Path to metadata.json file
/// 
/// # Errors
/// Returns error if file not found or JSON invalid
pub fn load(path: &Path) -> Result<Self> {
```
- [ ] All public functions documented
- [ ] All public structs documented
- [ ] All public fields documented
- [ ] Examples provided for complex functions

#### Subtask 1.1.5.2: Update Weekly Progress
Update BUILD_PHASE_IMPLEMENTATION.md:
```
### Week 1 Progress

#### Completed
- [x] MetadataManager implementation (8 tests)
- [x] ChecksumGenerator implementation (2 tests)
- [x] Module integration
- [x] 10/10 tests passing
- [x] Code documentation complete

#### Time Spent
- Metadata: 3.5 hours
- Checksum: 1.5 hours
- Integration: 0.5 hours
- Total: 5.5 hours (under 6 hour estimate)

#### Quality Metrics
- Tests: 10 passing
- Coverage: 95%+
- Compiler warnings: 0
```
- [ ] Progress documented
- [ ] Time tracked
- [ ] Quality metrics recorded

#### Subtask 1.1.5.3: Code Review Checklist
Before moving to Week 2:
- [ ] All tests passing: `cargo test --lib builder`
- [ ] No compiler warnings: `cargo build --release`
- [ ] Code formatted: `cargo fmt`
- [ ] Clippy passes: `cargo clippy --lib`
- [ ] Documentation builds: `cargo doc --no-deps --open`

**Estimated time**: 1-2 hours  
**Status**: ⏳ Waiting to start  

---

## Week 1 Summary

### Expected Deliverables
```
✅ src/builder/metadata.rs (~250 lines)
✅ src/builder/checksum.rs (~150 lines)
✅ src/builder/mod.rs (updated)
✅ src/lib.rs (updated)
✅ 10 passing tests
✅ Documentation complete
✅ Code coverage >90%
```

### Time Budget
- Metadata implementation: 3-4 hours
- Checksum implementation: 1-2 hours
- Integration & testing: 1-2 hours
- Documentation: 1 hour
- **TOTAL**: 6-9 hours (assuming 1.5 hour buffer)

### Success Criteria
- [ ] All 10 tests passing
- [ ] `cargo build --release` succeeds
- [ ] No compiler warnings
- [ ] Code formatted and clean
- [ ] Ready for Week 2 (TarballBuilder)

---

## How to Track Progress

### Daily Standup (for yourself)
Each day, update this section:
```markdown
## Daily Progress

### Monday [Date]
- Completed: Metadata struct, validation logic
- Time spent: 2 hours
- Blockers: None
- Next: Implement version bumping

### Tuesday [Date]
- Completed: Version bumping, utility methods
- Time spent: 1.5 hours
- Blockers: None
- Next: Tests

### Wednesday [Date]
- ...
```

### Tests as You Go
After each subtask:
```bash
cargo test --lib builder
```

Don't wait until end of week to run tests.

---

## If You Get Stuck

### Compilation Errors?
1. Check Cargo.toml for missing dependencies
2. Check imports: `use std::fs;`, `use anyhow::{Result};`
3. Run `cargo check` for more details

### Test Failures?
1. Read error message carefully
2. Add debug prints: `println!("{:?}", variable);`
3. Check test data setup
4. Run single test: `cargo test --lib builder::metadata::tests::test_name`

### Performance Issues?
1. Check file I/O buffering (8KB chunks)
2. Avoid allocating large vectors
3. Use iterators instead of collecting

### General Questions?
See: BUILD_PHASE_IMPLEMENTATION.md for detailed implementation notes

---

## Tools & Commands Reference

```bash
# Compile and check
cargo check
cargo build
cargo build --release

# Test
cargo test --lib
cargo test --lib builder
cargo test --lib builder::metadata
cargo test --lib builder::checksum

# Quality
cargo fmt                    # Format code
cargo clippy --lib          # Lint
cargo doc --no-deps --open  # Generate docs

# Performance
cargo build --release --timings

# Git workflow (optional)
git add src/builder/
git commit -m "feat: Add metadata and checksum modules"
```

---

## After Week 1 Complete

When all Week 1 tasks ✅ complete and tests ✅ passing:

1. Update IMPLEMENTATION_PRIORITY.md with "Week 1 Complete"
2. Create Weekly Summary in BUILD_PHASE_IMPLEMENTATION.md
3. Start Week 2: TarballBuilder
4. Celebrate! 🎉

---

**Start Date**: [Fill in when you start]  
**Target Completion**: End of Week 1  
**Current Status**: 🚀 READY TO BEGIN
