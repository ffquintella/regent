// Artichoke Ruby Interoperability Module
// This module enables seamless integration between Rust and Ruby code

use artichoke::prelude::RubyException;
use artichoke::prelude::*;
use std::borrow::Cow;
use std::path::Path;

/// Ruby execution environment
pub struct RubyEnvironment {
    interp: artichoke::Artichoke,
}

impl RubyEnvironment {
    pub fn new() -> anyhow::Result<Self> {
        eprintln!("Artichoke init start");
        let interp = artichoke::interpreter().map_err(|err| anyhow::anyhow!(err.to_string()))?;
        eprintln!("Artichoke init ok");
        Ok(Self { interp })
    }

    /// Execute Ruby code from Rust
    pub fn eval(&mut self, code: &str) -> anyhow::Result<()> {
        eprintln!("Artichoke eval start ({} bytes)", code.len());
        match self.interp.eval(code.as_bytes()) {
            Ok(_) => Ok(()),
            Err(err) => Err(anyhow::anyhow!(self.format_error(err))),
        }?;
        eprintln!("Artichoke eval ok");
        Ok(())
    }

    /// Execute Ruby code and return its string value.
    pub fn eval_to_string(&mut self, code: &str) -> anyhow::Result<String> {
        eprintln!("Artichoke eval_to_string start ({} bytes)", code.len());
        let value = match self.interp.eval(code.as_bytes()) {
            Ok(value) => value,
            Err(err) => return Err(anyhow::anyhow!(self.format_error(err))),
        };
        let value = value
            .try_convert_into_mut::<String>(&mut self.interp)
            .map_err(|err| anyhow::anyhow!(err.to_string()))?;
        eprintln!("Artichoke eval_to_string ok");
        Ok(value)
    }

    /// Register a Ruby source file in the Artichoke virtual file system.
    pub fn def_rb_source_file<P: AsRef<Path>>(
        &mut self,
        path: P,
        contents: Vec<u8>,
    ) -> anyhow::Result<()> {
        self.interp
            .def_rb_source_file(path, Cow::Owned(contents))
            .map_err(|err| anyhow::anyhow!(err.to_string()))?;
        Ok(())
    }

    /// Check if a Ruby source file exists in the virtual file system.
    pub fn source_is_file<P: AsRef<Path>>(&mut self, path: P) -> anyhow::Result<bool> {
        let exists = self
            .interp
            .source_is_file(path)
            .map_err(|err| anyhow::anyhow!(err.to_string()))?;
        Ok(exists)
    }

    /// Call Ruby function from Rust
    pub fn call_function(&mut self, name: &str, args: Vec<String>) -> anyhow::Result<String> {
        let args_literal = args
            .into_iter()
            .map(|value| format!("{value:?}"))
            .collect::<Vec<String>>()
            .join(", ");
        let expr = format!("{name}({args_literal})");
        let value = match self.interp.eval(expr.as_bytes()) {
            Ok(value) => value,
            Err(err) => return Err(anyhow::anyhow!(self.format_error(err))),
        };
        let value = value
            .try_convert_into_mut::<String>(&mut self.interp)
            .map_err(|err| anyhow::anyhow!(err.to_string()))?;
        Ok(value)
    }

    /// Load Ruby gem compatibility layer
    pub fn load_gem(&mut self, gem_name: &str) -> anyhow::Result<()> {
        let script = format!("require {gem_name:?}");
        match self.interp.eval(script.as_bytes()) {
            Ok(_) => Ok(()),
            Err(err) => Err(anyhow::anyhow!(self.format_error(err))),
        }?;
        Ok(())
    }

    fn format_error(&mut self, err: artichoke::Error) -> String {
        let name = err.name().to_string();
        let message = err.message();
        let message = String::from_utf8_lossy(message.as_ref());
        let mut output = format!("{name}: {message}");
        if let Some(backtrace) = err.vm_backtrace(&mut self.interp) {
            let frames = backtrace
                .into_iter()
                .map(|frame| String::from_utf8_lossy(&frame).to_string())
                .collect::<Vec<String>>();
            if !frames.is_empty() {
                output.push('\n');
                output.push_str(&frames.join("\n"));
            }
        }
        output
    }
}

impl Default for RubyEnvironment {
    fn default() -> Self {
        Self::new().unwrap_or_else(|err| panic!("failed to initialize Artichoke: {err}"))
    }
}

#[cfg(test)]
mod merge_repro_tests {
    use super::*;
    use crate::tester::artichoke_runner::HASH_MERGE_FIX;

    #[test]
    fn hash_merge_fix_handles_all_call_forms() {
        let mut env = RubyEnvironment::new().unwrap();
        env.eval(HASH_MERGE_FIX).unwrap();
        // explicit braces
        assert_eq!(
            env.eval_to_string("({'a' => 1}.merge({'b' => 2})).length.to_s")
                .unwrap(),
            "2"
        );
        // implicit hash with hashrocket
        assert_eq!(
            env.eval_to_string("({'a' => 1}.merge('b' => 2)).length.to_s")
                .unwrap(),
            "2"
        );
        // bare symbol keyword — the originally-failing form
        assert_eq!(
            env.eval_to_string("({'a' => 1}.merge(b: 2)).length.to_s")
                .unwrap(),
            "2"
        );
        // the merged value is actually present and correct (symbol key, as MRI)
        assert_eq!(
            env.eval_to_string("({'a' => 1}.merge(b: 2))[:b].to_s")
                .unwrap(),
            "2"
        );
        // merge! / update in place
        assert_eq!(
            env.eval_to_string("(h = {'a' => 1}; h.update(b: 2); h.length).to_s")
                .unwrap(),
            "2"
        );
    }
}

/// FFI bridge for calling Rust from Ruby
pub mod ffi {
    pub fn rust_function_from_ruby(_arg: String) -> String {
        "Result from Rust".to_string()
    }
}
