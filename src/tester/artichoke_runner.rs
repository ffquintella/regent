use anyhow::Result;
use std::path::PathBuf;
use walkdir::WalkDir;

use super::bundled_gems::{discover_bundle_roots, ensure_user_bundle};
use super::{RegentPlan, RegentSpecRunner, TestCase, TestConfig, TestResults, TestStatus};
use crate::ruby_interop::RubyEnvironment;

const VIRTUAL_ROOT: &str = "/artichoke/virtual_root/src/lib";

/// Pure-Ruby reimplementation of `Hash#merge`, `Hash#merge!`, and `Hash#update`.
///
/// Artichoke's Rust-backed `Hash#merge` rejects a trailing implicit hash —
/// both `h.merge(k: v)` (bare symbol keywords) and `h.merge('k' => v)`
/// (hashrocket) raise a self-contradictory `ArgumentError: wrong number of
/// arguments (given 1, expected 1)`; only `h.merge({ ... })` with explicit
/// braces works. Re-defining these in Ruby with `(*others, **kwargs)` lets the
/// interpreter's normal call-binding fold the implicit hash / keyword bucket
/// back in (MRI's behavior for methods that don't declare keywords), fixing
/// every variadic last-hash call site (`merge`, `update`, fact-hash builders,
/// etc.) at once.
pub(crate) const HASH_MERGE_FIX: &str = r#"
class Hash
  def merge(*others, **kwargs)
    result = {}
    each { |k, v| result[k] = v }
    others.each do |other|
      other.each { |k, v| result[k] = v } if other
    end
    kwargs.each { |k, v| result[k] = v }
    result
  end

  def merge!(*others, **kwargs)
    others.each do |other|
      other.each { |k, v| self[k] = v } if other
    end
    kwargs.each { |k, v| self[k] = v }
    self
  end
  alias update merge!
end
"#;

/// Artichoke-backed test runner (no system Ruby/Puppet dependency).
pub struct ArtichokeTestRunner<'a> {
    config: &'a TestConfig,
}

impl<'a> ArtichokeTestRunner<'a> {
    pub fn new(config: &'a TestConfig) -> Self {
        Self { config }
    }

    pub fn run_unit_tests(&self) -> Result<TestResults> {
        let mut results = TestResults::new("unit");

        if std::env::var("REGENT_DISABLE_ARTICHOKE").is_ok() {
            results.exit_code = 1;
            results.failed = 1;
            results.stderr = "Artichoke disabled via REGENT_DISABLE_ARTICHOKE".to_string();
            return Ok(results);
        }

        eprintln!("Artichoke runner: ensure bundled gems");
        let _ = ensure_user_bundle()?;

        let spec_dir = self.config.module_path.join("spec");
        if !spec_dir.exists() {
            return Ok(results);
        }

        eprintln!("Artichoke runner: discover spec files");
        let mut spec_files = Vec::new();
        for entry in WalkDir::new(&spec_dir).into_iter().flatten() {
            let path = entry.path();
            if path.is_file()
                && path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .map_or(false, |name| name.ends_with("_spec.rb"))
            {
                spec_files.push(path.to_path_buf());
            }
        }

        eprintln!("Artichoke runner: build load paths");
        let mut load_paths = self.build_load_paths();
        let smoke = std::env::var("REGENT_RSPEC_SMOKE").is_ok();
        let use_real_rspec = std::env::var("REGENT_RSPEC_REAL").is_ok();
        let use_regent = !use_real_rspec && !smoke;
        if smoke {
            load_paths.retain(|path| path.to_string_lossy().contains("vendor/bundle"));
        }
        eprintln!("Artichoke runner: map spec files");
        let virtual_spec_files = self.build_virtual_spec_files(&spec_files, &spec_dir);
        let supported_os = self.load_supported_os();
        let ruby_script = if smoke {
            self.build_rspec_smoke_script(&load_paths)
        } else if use_regent {
            self.build_regent_plan_script(&load_paths, &virtual_spec_files, &supported_os)
        } else {
            self.build_rspec_script(&load_paths, &virtual_spec_files)
        };

        eprintln!("Artichoke runner: create interpreter");
        let mut env = RubyEnvironment::new()?;
        eprintln!("Artichoke runner: seed stdlib");
        self.seed_stdlib(&mut env)?;
        eprintln!("Artichoke runner: load ruby sources");
        self.load_ruby_sources(&mut env, &load_paths)?;
        if use_real_rspec {
            let rspec_virtual = PathBuf::from(VIRTUAL_ROOT).join("rspec.rb");
            let rspec_available = env.source_is_file(&rspec_virtual).unwrap_or(false);
            if !rspec_available {
                results.exit_code = 1;
                results.total = if smoke { 1 } else { spec_files.len() };
                results.failed = if smoke { 1 } else { spec_files.len() };
                results.stderr = format!(
                    "Artichoke could not find rspec.rb at {}.\nRun `regent bootstrap` in your module directory to install Regent's required gems.",
                    rspec_virtual.display()
                );
                return Ok(results);
            }
            let comparable_virtual = PathBuf::from(VIRTUAL_ROOT)
                .join("support")
                .join("comparable_version.rb");
            if !env.source_is_file(&comparable_virtual).unwrap_or(false) {
                results.exit_code = 1;
                results.total = if smoke { 1 } else { spec_files.len() };
                results.failed = if smoke { 1 } else { spec_files.len() };
                results.stderr = format!(
                    "Artichoke could not find support/comparable_version.rb at {}",
                    comparable_virtual.display()
                );
                return Ok(results);
            }
        }
        eprintln!("Artichoke runner: run tests");
        let output = match env.eval_to_string(&ruby_script) {
            Ok(output) => output,
            Err(err) => {
                results.exit_code = 1;
                results.total = if smoke { 1 } else { spec_files.len() };
                results.failed = if smoke { 1 } else { spec_files.len() };
                results.stderr = err.to_string();
                return Ok(results);
            }
        };

        if use_regent {
            let trimmed = output.trim_start();
            let plan: RegentPlan = if trimmed.starts_with('{') || trimmed.starts_with('[') {
                match serde_json::from_str(&output) {
                    Ok(plan) => plan,
                    Err(err) => {
                        results.exit_code = 1;
                        results.total = spec_files.len();
                        results.failed = spec_files.len();
                        results.stderr = format!("failed to parse regent plan: {err}\n{output}");
                        return Ok(results);
                    }
                }
            } else {
                // The plan script's top-level rescue returns a non-JSON
                // "ClassName: message\nbacktrace" string. Surface that as the
                // Ruby error it is, instead of burying it inside a misleading
                // JSON parse error.
                results.exit_code = 1;
                results.total = spec_files.len();
                results.failed = spec_files.len();
                results.stderr = output;
                return Ok(results);
            };
            let runner = RegentSpecRunner::new(&self.config.module_path)?;
            let regent_results = runner.run_plan(plan)?;
            return Ok(regent_results);
        }

        let summary = Self::parse_summary(&output);
        results.total = summary.total;
        results.failed = summary.failed;
        results.pending = summary.pending;
        results.passed = summary.passed;
        results.exit_code = summary.exit_code;

        if smoke {
            results.add_test_case(TestCase {
                name: "rspec_smoke".to_string(),
                status: if summary.exit_code == 0 {
                    TestStatus::Passed
                } else {
                    TestStatus::Failed
                },
                duration_ms: 0,
                message: None,
            });
        } else {
            for path in spec_files {
                results.add_test_case(TestCase {
                    name: path.display().to_string(),
                    status: if summary.exit_code == 0 {
                        TestStatus::Passed
                    } else {
                        TestStatus::Failed
                    },
                    duration_ms: 0,
                    message: None,
                });
            }
        }

        results.stdout = summary.stdout;
        results.stderr = summary.stderr;
        Ok(results)
    }

    fn load_supported_os(&self) -> Vec<SupportedOs> {
        let metadata_path = self.config.module_path.join("metadata.json");
        let Ok(contents) = std::fs::read_to_string(&metadata_path) else {
            return Vec::new();
        };
        let Ok(value) = serde_json::from_str::<serde_json::Value>(&contents) else {
            return Vec::new();
        };
        let Some(entries) = value
            .get("operatingsystem_support")
            .and_then(|v| v.as_array())
        else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for entry in entries {
            let Some(os) = entry.get("operatingsystem").and_then(|v| v.as_str()) else {
                continue;
            };
            let releases: Vec<String> = entry
                .get("operatingsystemrelease")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(str::to_string))
                        .collect()
                })
                .unwrap_or_default();
            if releases.is_empty() {
                out.push(SupportedOs::new(os, ""));
            } else {
                for release in releases {
                    out.push(SupportedOs::new(os, &release));
                }
            }
        }
        out
    }

    fn build_load_paths(&self) -> Vec<PathBuf> {
        let mut paths = Vec::new();
        let mut roots: Vec<PathBuf> = discover_bundle_roots();
        // Legacy per-module location, for users that previously ran the older
        // bootstrap which wrote into the module's vendor/bundle.
        roots.push(self.config.module_path.join("vendor").join("bundle"));
        for root in roots {
            let ruby_root = root.join("ruby");
            let Ok(entries) = std::fs::read_dir(&ruby_root) else {
                continue;
            };
            for entry in entries.flatten() {
                let gem_root = entry.path().join("gems");
                let Ok(gems) = std::fs::read_dir(gem_root) else {
                    continue;
                };
                for gem in gems.flatten() {
                    let lib = gem.path().join("lib");
                    if lib.is_dir() {
                        paths.push(lib);
                    }
                }
            }
        }
        paths.push(self.config.module_path.join("lib"));
        paths.push(self.config.module_path.join("spec"));
        paths
    }

    fn build_rspec_script(&self, _load_paths: &[PathBuf], spec_files: &[PathBuf]) -> String {
        let load_path_literal = format!("{:?}", VIRTUAL_ROOT);
        let spec_file_literals = spec_files
            .iter()
            .map(|path| format!("{:?}", path.display().to_string()))
            .collect::<Vec<String>>()
            .join(", ");
        format!(
            r##"
begin
  stderr = ''
  $LOAD_PATH = [{load_path_literal}]
  require 'marshal'
  class IO
    attr_accessor :sync

    def initialize
      @buffer = ""
      @sync = false
    end

    def write(value)
      @buffer << value.to_s
    end

    def puts(value = "")
      @buffer << value.to_s
      @buffer << "\n"
    end

    def string
      @buffer
    end
  end
  class Object
    def instance_exec(*_args, &block)
      instance_eval(&block)
    end
  end
  class Module
    def module_function(*names)
      if names.empty?
        @__regent_module_function = true
        return
      end
      names.each do |name|
        original = instance_method(name)
        singleton_class.send(:define_method, name) do |*args, &block|
          original.bind(self).call(*args, &block)
        end
        private name
      end
    end

    def method_added(name)
      if @__regent_module_function
        singleton_class.send(:define_method, name, instance_method(name))
        private name
      end
    end
  end
  class String
    begin
      undef_method :to_f
    rescue
    end
    def to_f
      parts = split(".", 2)
      return parts[0].to_i if parts.length == 1
      integer = parts[0].to_i
      fraction = parts[1].to_i
      integer + (fraction / (10.0 ** parts[1].length))
    end
  end
  class Array
    def max
      return nil if empty?
      current = first
      each do |value|
        current = value if (current <=> value) == -1
      end
      current
    end
  end
  class Module
    def autoload(_name, path)
      require path
    end

    def private_class_method(*names)
      names.each do |name|
        singleton_class.send(:private, name)
      end
    end

    def public_class_method(*names)
      names.each do |name|
        singleton_class.send(:public, name)
      end
    end

    def public_instance_methods(*_args)
      instance_methods
    end
  end
  class Struct
    def self.new(*members, &block)
      klass = Class.new do
        members.each do |member|
          define_method(member) do
            instance_variable_get("@#{{member}}")
          end
          define_method("#{{member}}=") do |value|
            instance_variable_set("@#{{member}}", value)
          end
        end

        define_method(:initialize) do |*values|
          members.each_with_index do |member, index|
            instance_variable_set("@#{{member}}", values[index])
          end
        end
      end
      klass.class_eval(&block) if block
      klass
    end
  end
  module Diff
  end
  class File
    ALT_SEPARATOR = nil

    def self.expand_path(path, base = nil)
      base = "/artichoke/virtual_root/src/lib" if base.nil? || base == "."
      if path.start_with?("/")
        normalize(path)
      else
        normalize("#{{base}}/#{{path}}")
      end
    end

    def self.join(*parts)
      normalize(parts.join("/"))
    end

    def self.dirname(path)
      path = normalize(path)
      parts = path.split("/")
      parts.pop
      return "/" if parts.empty?
      parts.join("/")
    end

    def self.basename(path)
      path.split("/").last
    end

    def self.normalize(path)
      stack = []
      path.split("/").each do |part|
        next if part.empty? || part == "."
        if part == ".."
          stack.pop
        else
          stack << part
        end
      end
      "/" + stack.join("/")
    end
  end
  unless Object.const_defined?(:Ruby) && ::Ruby.respond_to?(:jruby?)
    module Ruby
      def self.jruby?
        false
      end

      def self.jruby_version
        "0.0.0"
      end

      def self.rbx?
        false
      end

      def self.truffleruby?
        false
      end

      def self.mri?
        true
      end

      def self.non_mri?
        false
      end
    end
  end
  module RSpec
    module Support
      module Ruby
        def self.jruby?
          false
        end

        def self.jruby_version
          "0.0.0"
        end

        def self.rbx?
          false
        end

        def self.truffleruby?
          false
        end

        def self.mri?
          true
        end

        def self.non_mri?
          false
        end
      end
      module RubyFeatures
        def self.supports_rebinding_module_methods?
          false
        end

        def self.caller_locations_supported?
          false
        end

        def self.ripper_supported?
          false
        end

        def self.supports_exception_cause?
          false
        end

        def self.module_prepends_supported?
          false
        end

        def self.optional_and_splat_args_supported?
          false
        end

        def self.kw_args_supported?
          false
        end

        def self.required_kw_args_supported?
          false
        end
      end
      module OS
        def self.windows?
          false
        end

        def self.windows_file_path?
          false
        end
      end
    end
  end
  module RSpec
    module Core
      module Formatters
        @__registrations = {{}}
        def self.register(formatter_class, *notifications)
          @__registrations[formatter_class] = notifications
        end

        def self.registrations
          @__registrations
        end
      end
    end
  end
  module Kernel
    unless method_defined?(:__regent_require_relative)
      alias __regent_require_relative require_relative if method_defined?(:require_relative)
      def require_relative(path)
        if !path.start_with?("/") && !path.start_with?("rspec/")
          begin
            return require("rspec/#{{path}}")
          rescue LoadError
          end
        end
        require(path)
      end
    end
  end
  module RSpec
    module Support
      RUBY_VERSION = Object.new
      def RUBY_VERSION.to_f
        3.1
      end
    end
  end
  class String
    def to_f
      parts = split(".", 2)
      return parts[0].to_i if parts.length == 1
      integer = parts[0].to_i
      fraction = parts[1].to_i
      integer + (fraction / (10.0 ** parts[1].length))
    end
  end
  class RegentVersion
    def initialize(value)
      @value = value
    end

    def to_f
      parts = @value.split(".", 2)
      return parts[0].to_i if parts.length == 1
      integer = parts[0].to_i
      fraction = parts[1].to_i
      integer + (fraction / (10.0 ** parts[1].length))
    end

    def to_s
      @value
    end
  end
  begin
    Object.send(:remove_const, :RUBY_VERSION)
  rescue
  end
  Object.const_set(:RUBY_VERSION, Object.new)
  def RUBY_VERSION.to_f
    3.1
  end
  def RUBY_VERSION.to_s
    "3.1.2"
  end
  module Kernel
    def private_class_method(*)
    end

    def public_class_method(*)
    end

    def module_function(*)
    end
  end
  module RSpec
    module Support
      module Ruby
        def self.jruby?
          false
        end

        def self.jruby_version
          "0.0.0"
        end

        def self.rbx?
          false
        end

        def self.truffleruby?
          false
        end

        def self.mri?
          true
        end

        def self.non_mri?
          false
        end
      end
    end
  end
  require 'rspec'
  require 'rspec/core'
  require 'rspec/core/runner'
  spec_files = [{spec_file_literals}]
  args = ['--format', 'progress']
  args.concat(spec_files)
  exit_code = RSpec::Core::Runner.run(args)
  total = RSpec.world.example_count
  failed = RSpec.world.reporter.failed_examples.count
  pending = RSpec.world.reporter.pending_examples.count
  passed = total - failed - pending
  [
    "exit_code=#{{exit_code}}",
    "total=#{{total}}",
    "failed=#{{failed}}",
    "pending=#{{pending}}",
    "passed=#{{passed}}",
    "stdout=",
    "stderr=#{{stderr}}"
  ].join("\n")
rescue => e
  backtrace = e.backtrace ? e.backtrace.join("\n") : ""
  stderr = "#{{e.class}}: #{{e.message}}\n#{{backtrace}}".gsub("\n", "\\n")
  [
    "exit_code=1",
    "total=0",
    "failed=1",
    "pending=0",
    "passed=0",
    "stdout=",
    "stderr=#{{stderr}}"
  ].join("\n")
end
"##
        )
    }

    fn build_regent_plan_script(
        &self,
        _load_paths: &[PathBuf],
        spec_files: &[PathBuf],
        supported_os: &[SupportedOs],
    ) -> String {
        let load_path_literal = format!("{:?}", VIRTUAL_ROOT);
        let spec_file_literals = spec_files
            .iter()
            .map(|path| format!("{:?}", path.display().to_string()))
            .collect::<Vec<String>>()
            .join(", ");
        let supported_os_literal = render_supported_os_ruby(supported_os);
        format!(
            r##"
begin
  stderr = ''
  $LOAD_PATH = [{load_path_literal}]
  $regent_skip_requires = [
    "spec_helper",
    "puppet",
    "puppet/util",
    "rspec-puppet",
    "puppetlabs_spec_helper",
    "puppetlabs_spec_helper/module_spec_helper"
  ]
  module Kernel
    alias __regent_require require
    def require(path)
      $regent_skip_requires.each do |skip|
        return true if path == skip || path.start_with?("#{{skip}}/")
      end
      __regent_require(path)
    end

    if method_defined?(:require_relative)
      alias __regent_require_relative require_relative
      def require_relative(path)
        $regent_skip_requires.each do |skip|
          return true if path == skip || path.start_with?("#{{skip}}/")
        end
        require(path)
      end
    end
  end
  module RegentSpec
    class Context
      attr_reader :description, :lets, :subject
      def initialize(description, subject = nil)
        @description = description
        @subject = subject
        @lets = {{}}
      end
    end

    class Example
      attr_accessor :name, :expectations, :facts, :params, :title, :subject
      def initialize(name)
        @name = name
        @expectations = []
        @facts = nil
        @params = nil
        @title = nil
        @subject = nil
      end
    end

    @contexts = []
    @shared_examples = {{}}
    @tests = []
    @example_index = 0
    @current_example = nil

    class << self
      attr_reader :tests
    end

    def self.push_context(description, subject = nil)
      @contexts << Context.new(description, subject)
      yield
      @contexts.pop
    end

    def self.register_shared(name, block)
      @shared_examples[name] = block
    end

    def self.include_shared(name)
      block = @shared_examples[name]
      instance_eval(&block) if block
    end

    def self.register_let(name, block)
      @contexts.last.lets[name] = block
    end

    def self.resolve_let(name)
      @contexts.reverse_each do |ctx|
        return instance_eval(&ctx.lets[name]) if ctx.lets.key?(name)
      end
      nil
    end

    def self.start_example(description)
      @example_index += 1
      prefix = @contexts.map(&:description).compact.join(" ")
      label = description || "example #{{@example_index}}"
      name = [prefix, label].reject(&:empty?).join(" ")
      @current_example = Example.new(name)
      yield
      @current_example.facts = normalize_value(resolve_let(:facts))
      @current_example.params = normalize_value(resolve_let(:params))
      @current_example.title = normalize_value(resolve_let(:title))
      @current_example.subject = resolve_subject
      @tests << @current_example
      @current_example = nil
    end

    def self.add_expectation(expectation)
      @current_example.expectations << expectation if @current_example
    end

    def self.resolve_subject
      # rspec-puppet derives the class/define under test from the *top-level*
      # describe (`top_level_description`), never from a nested describe/context
      # used purely for grouping. Iterate outermost-first and take the first
      # subject so `describe 'firewallmanager' do ... describe 'on RedHat 10' do`
      # compiles `firewallmanager`, not the nested label.
      @contexts.each do |ctx|
        return ctx.subject if ctx.subject
      end
      nil
    end

    def self.build_plan
      @tests.map do |example|
        {{
          "name" => example.name,
          "subject" => example.subject,
          "title" => example.title,
          "facts" => example.facts,
          "params" => example.params,
          "expectations" => example.expectations.map do |exp|
            case exp.kind
            when "compile"
              {{ "kind" => "compile", "negate" => exp.negate }}
            when "raise_error"
              {{
                "kind" => "raise_error",
                "message" => normalize_value(exp.error_message),
                "negate" => exp.negate
              }}
            else
              {{
                "kind" => "contain",
                "resource_type" => exp.resource_type,
                "title" => exp.title,
                "attributes" => normalize_value(exp.attributes || {{}}),
                "negate" => exp.negate
              }}
            end
          end
        }}
      end
    end

    def self.normalize_value(value)
      case value
      when Hash
        value.each_with_object({{}}) do |(key, val), acc|
          acc[key.to_s] = normalize_value(val)
        end
      when Array
        value.map {{ |item| normalize_value(item) }}
      when Symbol
        value.to_s
      when Regexp
        {{ "__regex__" => value.source }}
      else
        value
      end
    end

    def self.to_json(value)
      case value
      when Hash
        items = value.map do |key, val|
          %Q("#{{escape_json(key.to_s)}}":#{{to_json(val)}})
        end
        "{{#{{items.join(',')}}}}"
      when Array
        items = value.map {{ |val| to_json(val) }}
        "[#{{items.join(',')}}]"
      when String
        %Q("#{{escape_json(value)}}")
      when TrueClass, FalseClass
        value ? "true" : "false"
      when NilClass
        "null"
      else
        value.to_s
      end
    end

    def self.escape_json(text)
      text.gsub(/\\/) {{ '\\\\' }}
          .gsub(/"/) {{ '\\"' }}
          .gsub(/\n/) {{ '\\n' }}
          .gsub(/\r/) {{ '\\r' }}
          .gsub(/\t/) {{ '\\t' }}
    end
  end

  class ExpectationTarget
    def to(matcher)
      matcher.instance_variable_set(:@negate, false) if matcher
      RegentSpec.add_expectation(matcher)
    end

    def not_to(matcher)
      matcher.instance_variable_set(:@negate, true) if matcher
      RegentSpec.add_expectation(matcher)
    end
    alias to_not not_to
  end

  class ContainMatcher
    attr_reader :resource_type, :title, :attributes, :absent_attributes

    def initialize(resource_type, title)
      @resource_type = resource_type
      @title = title
      @attributes = {{}}
      @absent_attributes = []
    end

    def with(attrs = nil, **kwargs)
      attrs = attrs || {{}}
      attrs = attrs.merge(kwargs) unless kwargs.empty?
      attrs.each {{ |k, v| @attributes[k.to_s] = v }}
      self
    end

    def without(attrs = nil, **kwargs)
      attrs = attrs || {{}}
      attrs = attrs.merge(kwargs) unless kwargs.empty?
      attrs.each {{ |k, _v| @absent_attributes << k.to_s }}
      self
    end

    def that_requires(*); self; end
    def that_comes_before(*); self; end
    def that_notifies(*); self; end
    def that_subscribes_to(*); self; end
    def only_with(attrs = nil, **kwargs)
      with(attrs, **kwargs)
    end

    def method_missing(name, *args, &block)
      str = name.to_s
      if str.start_with?("with_")
        attr = str.sub("with_", "")
        @attributes[attr] = args.length == 1 ? args.first : args
        self
      elsif str.start_with?("without_")
        attr = str.sub("without_", "")
        @absent_attributes << attr
        self
      elsif str.start_with?("only_with_")
        attr = str.sub("only_with_", "")
        @attributes[attr] = args.length == 1 ? args.first : args
        self
      else
        super
      end
    end

    def respond_to_missing?(name, include_private = false)
      str = name.to_s
      str.start_with?("with_") || str.start_with?("without_") || str.start_with?("only_with_") || super
    end

    def kind
      "contain"
    end

    def negate
      @negate ? true : false
    end
  end

  class CompileMatcher
    attr_reader :error_message

    # `compile.and_raise_error(/msg/)` — the compile is expected to fail.
    # Only a Regexp/String argument (the message constraint) is retained; an
    # exception-class argument is accepted but ignored.
    def and_raise_error(*args)
      @expect_error = true
      args.each {{ |a| @error_message = a if a.is_a?(Regexp) || a.is_a?(String) }}
      self
    end

    def kind
      @expect_error ? "raise_error" : "compile"
    end

    def negate
      @negate ? true : false
    end
  end

  # `expect {{ ... }}.to raise_error(Klass, /msg/)`. The block is irrelevant to
  # us: every example already compiles its subject in Rust, and that is what
  # the block triggers (`catalogue`, `is_expected.to compile`, etc.).
  class RaiseErrorMatcher
    attr_reader :error_message

    def initialize(message = nil)
      @error_message = message
    end

    def with_message(message)
      @error_message = message
      self
    end

    def kind
      "raise_error"
    end

    def negate
      @negate ? true : false
    end
  end

  def describe(subject, &block)
    RegentSpec.push_context(subject.to_s, subject.to_s) do
      instance_eval(&block)
    end
  end
  def context(subject, &block)
    RegentSpec.push_context(subject.to_s, nil) do
      instance_eval(&block)
    end
  end
  def shared_examples(name, &block)
    RegentSpec.register_shared(name.to_s, block)
  end
  def include_examples(name)
    RegentSpec.include_shared(name.to_s)
  end
  def let(name, &block)
    RegentSpec.register_let(name.to_sym, block)
  end
  def it(description = nil, &block)
    RegentSpec.start_example(description.to_s.empty? ? nil : description.to_s) do
      instance_eval(&block)
    end
  end
  def specify(description = nil, &block)
    it(description, &block)
  end
  def is_expected
    ExpectationTarget.new
  end
  def compile
    CompileMatcher.new
  end
  def contain_class(title)
    ContainMatcher.new("class", title.to_s)
  end
  def contain_file(title)
    ContainMatcher.new("file", title.to_s)
  end
  def contain_package(title)
    ContainMatcher.new("package", title.to_s)
  end
  def contain_service(title)
    ContainMatcher.new("service", title.to_s)
  end
  def contain_exec(title)
    ContainMatcher.new("exec", title.to_s)
  end
  def contain_user(title)
    ContainMatcher.new("user", title.to_s)
  end
  def contain_group(title)
    ContainMatcher.new("group", title.to_s)
  end
  def contain_cron(title)
    ContainMatcher.new("cron", title.to_s)
  end
  def contain_mount(title)
    ContainMatcher.new("mount", title.to_s)
  end
  def contain_notify(title)
    ContainMatcher.new("notify", title.to_s)
  end
  def contain_host(title)
    ContainMatcher.new("host", title.to_s)
  end
  def contain_yumrepo(title)
    ContainMatcher.new("yumrepo", title.to_s)
  end
  def contain_apt__source(title)
    ContainMatcher.new("apt::source", title.to_s)
  end
  def contain_docker__run(title)
    ContainMatcher.new("docker::run", title.to_s)
  end
  def contain_docker_network(title)
    ContainMatcher.new("docker_network", title.to_s)
  end
  # Dynamic fallback for `contain_<type>` matchers so that defined types
  # (e.g. `contain_apache__vhost('foo')` -> `apache::vhost`) and any other
  # resource type not hardcoded above produce a working ContainMatcher.
  # `__` in the method name maps to `::` in the resource type, matching the
  # rspec-puppet convention.
  def method_missing(name, *args, &block)
    str = name.to_s
    if str.start_with?("contain_") && args.length == 1
      resource_type = str.sub("contain_", "").gsub("__", "::")
      return ContainMatcher.new(resource_type, args.first.to_s)
    end
    super
  end
  def respond_to_missing?(name, include_private = false)
    name.to_s.start_with?("contain_") || super
  end
  def before(*); end
  def after(*); end
  def subject(*); end
  # Block form `expect {{ ... }}.to raise_error(...)`: the block and any value
  # argument are ignored; the returned target only meaningfully accepts a
  # RaiseErrorMatcher (other matchers are handled via `is_expected`).
  def expect(*_args, &_block)
    ExpectationTarget.new
  end
  def raise_error(*args)
    message = nil
    args.each {{ |a| message = a if a.is_a?(Regexp) || a.is_a?(String) }}
    RaiseErrorMatcher.new(message)
  end
  def raise_exception(*args)
    raise_error(*args)
  end
  # rspec-puppet-facts stub: returns an OS hash derived from the module's
  # metadata.json (operatingsystem_support), so generated specs that wrap
  # their examples in `on_supported_os.each do |os, os_facts|` produce one
  # context per supported OS/release. Falls back to a single "default" entry
  # if metadata.json is missing, malformed, or empty.
  REGENT_SUPPORTED_OS = {supported_os_literal}
  def on_supported_os(_opts = nil)
    REGENT_SUPPORTED_OS
  end
  def private_class_method(*); end
  def public_class_method(*); end
  def module_function(*); end
  module Gem
    def self.loaded_specs
      {{}}
    end

    class Version
      include Comparable

      def initialize(value)
        @value = value.to_s
      end

      def <=>(other)
        other = other.to_s
        @value <=> other
      end

      def to_s
        @value
      end
    end
  end
  module RSpec
    def self.configure; end
  end
  # Puppet exception hierarchy so specs that name a class in `raise_error`
  # (e.g. `raise_error(Puppet::Error, /msg/)`) load. The class is not used to
  # match — the evaluator surfaces a single generic compile error — but the
  # constant must resolve.
  module Puppet
    class Error < StandardError; end
    class ParseError < Error; end
    class PreformattedError < ParseError; end
    class ParseErrorWithIssue < PreformattedError; end
    class ResourceError < Error; end
    class DevError < Error; end
  end
  failures = []
  spec_files = [{spec_file_literals}]
  spec_files.each do |path|
    begin
      require path
    rescue => e
      backtrace = e.backtrace ? e.backtrace.join("\n") : ""
      failures << "#{{path}}: #{{e.class}}: #{{e.message}}\n#{{backtrace}}"
    end
  end
  plan = {{ "tests" => RegentSpec.build_plan }}
  if failures.any?
    raise failures.join("\\n")
  end
  RegentSpec.to_json(plan)
rescue => e
  backtrace = e.backtrace ? e.backtrace.join("\n") : ""
  "#{{e.class}}: #{{e.message}}\n#{{backtrace}}"
end
"##
        )
    }

    fn build_rspec_smoke_script(&self, _load_paths: &[PathBuf]) -> String {
        let load_path_literal = format!("{:?}", VIRTUAL_ROOT);
        format!(
            r##"
begin
  stderr = ''
  $LOAD_PATH = [{load_path_literal}]
  require 'marshal'
  module Kernel
    unless method_defined?(:__regent_require_relative)
      alias __regent_require_relative require_relative if method_defined?(:require_relative)
      def require_relative(path)
        if !path.start_with?("/") && !path.start_with?("rspec/")
          begin
            return require("rspec/#{{path}}")
          rescue LoadError
          end
        end
        require(path)
      end
    end
  end
  require 'rspec'
  [
    "exit_code=0",
    "total=1",
    "failed=0",
    "pending=0",
    "passed=1",
    "stdout=",
    "stderr="
  ].join("\n")
rescue => e
  backtrace = e.backtrace ? e.backtrace.join("\n") : ""
  stderr = "#{{e.class}}: #{{e.message}}\n#{{backtrace}}".gsub("\n", "\\n")
  [
    "exit_code=1",
    "total=1",
    "failed=1",
    "pending=0",
    "passed=0",
    "stdout=",
    "stderr=#{{stderr}}"
  ].join("\n")
end
"##
        )
    }

    fn build_virtual_spec_files(&self, spec_files: &[PathBuf], spec_dir: &PathBuf) -> Vec<PathBuf> {
        let mut virtual_paths = Vec::new();
        for path in spec_files {
            let relative = match path.strip_prefix(spec_dir) {
                Ok(relative) => relative,
                Err(_) => continue,
            };
            virtual_paths.push(PathBuf::from(VIRTUAL_ROOT).join(relative));
        }
        virtual_paths
    }

    fn load_ruby_sources(&self, env: &mut RubyEnvironment, load_paths: &[PathBuf]) -> Result<()> {
        for root in load_paths {
            if !root.is_dir() {
                continue;
            }
            for entry in WalkDir::new(root).into_iter().flatten() {
                let path = entry.path();
                if path.is_file() && path.extension().and_then(|ext| ext.to_str()) == Some("rb") {
                    let relative = match path.strip_prefix(root) {
                        Ok(relative) => relative,
                        Err(_) => continue,
                    };
                    let virtual_path = PathBuf::from(VIRTUAL_ROOT).join(relative);
                    let exists = match env.source_is_file(&virtual_path) {
                        Ok(exists) => exists,
                        Err(_) => continue,
                    };
                    if exists {
                        continue;
                    }
                    let contents = match std::fs::read(path) {
                        Ok(contents) => contents,
                        Err(_) => continue,
                    };
                    let alias_contents = relative
                        .strip_prefix("rspec")
                        .ok()
                        .map(|_| contents.clone());
                    if env.def_rb_source_file(virtual_path, contents).is_err() {
                        continue;
                    }
                    if let (Ok(stripped), Some(alias_contents)) =
                        (relative.strip_prefix("rspec"), alias_contents)
                    {
                        let alias_path = PathBuf::from(VIRTUAL_ROOT).join(stripped);
                        let exists = env.source_is_file(&alias_path).unwrap_or(false);
                        if !exists {
                            let _ = env.def_rb_source_file(alias_path, alias_contents);
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn seed_stdlib(&self, env: &mut RubyEnvironment) -> Result<()> {
        let smoke = std::env::var("REGENT_RSPEC_SMOKE").is_ok();

        let rbconfig = r#"
module RbConfig
  CONFIG = {
    "ruby_version" => "3.1.2",
    "RUBY_INSTALL_NAME" => "artichoke",
    "ruby_install_name" => "artichoke",
    "bindir" => "/usr/local/bin",
    "EXEEXT" => "",
    "host_os" => "darwin",
    "arch" => "x86_64-darwin",
    "sitelibdir" => "/artichoke/virtual_root/src/lib"
  }
  MAKEFILE_CONFIG = CONFIG

  def self.ruby
    "artichoke"
  end

  def self.expand(value, _config = CONFIG)
    value
  end
end
"#;
        let rbconfig_path = PathBuf::from(VIRTUAL_ROOT).join("rbconfig.rb");
        env.def_rb_source_file(rbconfig_path, rbconfig.as_bytes().to_vec())?;

        let sizeof = r#"
module RbConfig
  SIZEOF = {}
end
"#;
        let sizeof_path = PathBuf::from(VIRTUAL_ROOT)
            .join("rbconfig")
            .join("sizeof.rb");
        env.def_rb_source_file(sizeof_path, sizeof.as_bytes().to_vec())?;

        let pp = r#"
module PP
  def self.pp(object, out = +"", _width = 79)
    out << object.inspect
  end
end
"#;
        let pp_path = PathBuf::from(VIRTUAL_ROOT).join("pp.rb");
        env.def_rb_source_file(pp_path, pp.as_bytes().to_vec())?;

        let diff_lcs = r#"
module Diff
  module LCS
    def self.diff(_a, _b)
      []
    end
  end
end
"#;
        let diff_lcs_path = PathBuf::from(VIRTUAL_ROOT).join("diff").join("lcs.rb");
        env.def_rb_source_file(diff_lcs_path, diff_lcs.as_bytes().to_vec())?;

        let diff_hunk = r#"
module Diff
  module LCS
    class Hunk
      attr_reader :file_length_difference

      def initialize(*)
        @file_length_difference = 0
      end
    end
  end
end
"#;
        let diff_hunk_path = PathBuf::from(VIRTUAL_ROOT)
            .join("diff")
            .join("lcs")
            .join("hunk.rb");
        env.def_rb_source_file(diff_hunk_path, diff_hunk.as_bytes().to_vec())?;

        let rspec_ruby_features = r#"
module RSpec
  module Support
    module Ruby
      def self.jruby?
        false
      end

      def self.jruby_version
        "0.0.0"
      end

      def self.rbx?
        false
      end

      def self.truffleruby?
        false
      end

      def self.mri?
        true
      end

      def self.non_mri?
        false
      end
    end

    module RubyFeatures
      def self.supports_rebinding_module_methods?
        false
      end

      def self.caller_locations_supported?
        false
      end

      def self.ripper_supported?
        false
      end

      def self.supports_exception_cause?
        false
      end

      def self.module_prepends_supported?
        false
      end

      def self.optional_and_splat_args_supported?
        false
      end

      def self.kw_args_supported?
        false
      end

      def self.required_kw_args_supported?
        false
      end
    end

    module OS
      def self.windows?
        false
      end

      def self.windows_file_path?
        false
      end
    end
  end
end
"#;
        let rspec_ruby_features_path = PathBuf::from(VIRTUAL_ROOT)
            .join("rspec")
            .join("support")
            .join("ruby_features.rb");
        env.def_rb_source_file(
            rspec_ruby_features_path,
            rspec_ruby_features.as_bytes().to_vec(),
        )?;

        if smoke {
            let rspec_support_stub = r#"
module RSpec
  module Support
    def self.define_optimized_require_for_rspec(_lib, &_block)
    end

    def self.require_rspec_support(*)
      true
    end

    def self.require_rspec_core(*)
      true
    end
  end
end
"#;
            let rspec_support_path = PathBuf::from(VIRTUAL_ROOT).join("rspec").join("support.rb");
            env.def_rb_source_file(rspec_support_path, rspec_support_stub.as_bytes().to_vec())?;

            let rspec_stub = r#"
module RSpec
  module Core
    module Runner
      def self.run(*)
        0
      end
    end
  end
end
"#;
            let rspec_stub_path = PathBuf::from(VIRTUAL_ROOT).join("rspec.rb");
            env.def_rb_source_file(rspec_stub_path, rspec_stub.as_bytes().to_vec())?;
        }

        let reentrant_mutex = r#"
module RSpec
  module Support
    class Mutex
      def initialize
        @locked = false
      end

      def lock
        @locked = true
        self
      end

      def unlock
        @locked = false
        self
      end

      def owned?
        @locked
      end
    end

    class ReentrantMutex
      def initialize
        @mutex = Mutex.new
        @count = 0
      end

      def synchronize
        enter
        yield
      ensure
        exit
      end

      def enter
        @mutex.lock unless @mutex.owned?
        @count += 1
      end

      def exit
        @count -= 1
        @mutex.unlock if @count <= 0
      end
    end
  end
end
"#;
        let reentrant_mutex_path = PathBuf::from(VIRTUAL_ROOT)
            .join("rspec")
            .join("support")
            .join("reentrant_mutex.rb");
        env.def_rb_source_file(reentrant_mutex_path, reentrant_mutex.as_bytes().to_vec())?;

        let stringio = r#"
class StringIO
  def initialize(str = "")
    @string = str.to_s
    @pos = 0
  end

  def write(value)
    @string << value.to_s
  end

  def <<(value)
    write(value)
  end

  def read(_len = nil)
    result = @string[@pos..-1] || ""
    @pos = @string.length
    result
  end

  def rewind
    @pos = 0
  end

  def string
    @string
  end

  def to_s
    @string
  end
end
"#;
        let stringio_path = PathBuf::from(VIRTUAL_ROOT).join("stringio.rb");
        env.def_rb_source_file(stringio_path, stringio.as_bytes().to_vec())?;

        // Real ERB: the template is compiled to Ruby that appends to a buffer,
        // then evaluated. Artichoke has no `binding`, so `result(b)` cannot see
        // a caller's locals — it evaluates against an empty context (instance
        // variables resolve to nil, bare names raise). `result_with_hash` binds
        // the hash keys as methods and works fully. Trim markers `<%-`/`-%>` are
        // always honored; the `>`/`<>` newline-suppression modes are not.
        let erb = r##"
class ERB
  attr_reader :src

  def initialize(str, *args)
    opts = args.last.is_a?(Hash) ? args[-1] : {}
    trim = opts[:trim_mode]
    trim = args[1] if trim.nil? && args.length >= 2 && !args[1].is_a?(Hash)
    @src = ERB.compile(str, trim)
  end

  def result(_binding = nil)
    Context.new({}).__erb_eval(@src)
  end

  def result_with_hash(hash)
    Context.new(hash).__erb_eval(@src)
  end

  def self.compile(template, _trim_mode = nil)
    s = template.to_s
    src = +"_erbout = +\"\"\n"
    i = 0
    n = s.length
    while i < n
      open = s.index("<%", i)
      if open.nil?
        _emit_text(src, s[i...n])
        break
      end
      if s[open + 2] == "%"
        _emit_text(src, s[i...open])
        _emit_text(src, "<%")
        i = open + 3
        next
      end
      marker = s[open + 2]
      left_trim = (marker == "-")
      text = s[i...open]
      text = text.sub(/[ \t]*\z/, "") if left_trim
      _emit_text(src, text)
      content_start = open + 2
      type = :code
      if marker == "="
        type = :expr
        content_start = open + 3
      elsif marker == "#"
        type = :comment
        content_start = open + 3
      elsif marker == "-"
        content_start = open + 3
      end
      close = s.index("%>", content_start)
      close = n if close.nil?
      code = s[content_start...close]
      right_trim = false
      if code.end_with?("-")
        right_trim = true
        code = code[0...-1]
      end
      case type
      when :expr
        src << "_erbout << (" << code << ").to_s\n"
      when :comment
        # dropped
      else
        src << code << "\n"
      end
      i = close >= n ? n : close + 2
      if right_trim
        j = i
        j += 1 while j < n && (s[j] == " " || s[j] == "\t")
        if j < n && s[j] == "\n"
          i = j + 1
        elsif j + 1 < n && s[j] == "\r" && s[j + 1] == "\n"
          i = j + 2
        end
      end
    end
    src << "_erbout\n"
    src
  end

  def self._emit_text(src, text)
    return if text.nil? || text.empty?
    lit = +"\""
    k = 0
    while k < text.length
      c = text[k]
      k += 1
      case c
      when "\\" then lit << "\\\\"
      when "\"" then lit << "\\\""
      when "\n" then lit << "\\n"
      when "\r" then lit << "\\r"
      when "\t" then lit << "\\t"
      when "#"  then lit << "\\#"
      else lit << c
      end
    end
    lit << "\""
    src << "_erbout << " << lit << "\n"
  end

  class Context
    def initialize(vars = {})
      @__vars = {}
      vars.each { |k, v| @__vars[k.to_sym] = v } if vars
    end

    def __erb_eval(code)
      instance_eval(code)
    end

    def method_missing(name, *args)
      return @__vars[name] if @__vars.key?(name)
      super
    end

    def respond_to_missing?(name, include_private = false)
      @__vars.key?(name) || super
    end
  end

  module Util
    def self.html_escape(value)
      value.to_s.gsub("&", "&amp;").gsub("<", "&lt;").gsub(">", "&gt;").gsub("\"", "&quot;").gsub("'", "&#39;")
    end

    def self.h(value)
      html_escape(value)
    end

    def self.url_encode(value)
      out = +""
      value.to_s.bytes.each do |b|
        ch = b.chr
        if (b >= 48 && b <= 57) || (b >= 65 && b <= 90) || (b >= 97 && b <= 122) ||
           ch == "-" || ch == "_" || ch == "." || ch == "~"
          out << ch
        else
          hex = b.to_s(16).upcase
          hex = "0" + hex if hex.length < 2
          out << "%" << hex
        end
      end
      out
    end

    def self.u(value)
      url_encode(value)
    end
  end
end
"##;
        let erb_path = PathBuf::from(VIRTUAL_ROOT).join("erb.rb");
        env.def_rb_source_file(erb_path, erb.as_bytes().to_vec())?;

        let drb = r#"
module DRb
  def self.start_service(*)
    true
  end

  def self.stop_service
    true
  end

  def self.thread
    nil
  end

  class DRbObject
    def self.new_with_uri(_uri)
      Object.new
    end
  end
end
"#;
        let drb_path = PathBuf::from(VIRTUAL_ROOT).join("drb").join("drb.rb");
        env.def_rb_source_file(drb_path, drb.as_bytes().to_vec())?;

        // Real Marshal for the core value types Puppet specs exchange (nil,
        // true/false, Integer, Float, String, Symbol, Array, Hash). It is NOT
        // the MRI binary format — it is a self-describing length-prefixed
        // encoding — but it round-trips (`load(dump(x)) == x`) and raises on
        // unsupported objects instead of silently corrupting them. Float
        // precision is limited to what `Float#to_s` preserves.
        let marshal = r#"
module Marshal
  MAJOR_VERSION = 4
  MINOR_VERSION = 8

  def self.dump(obj, *_args)
    out = +""
    _dump(obj, out)
    out
  end

  def self._dump(obj, out)
    if obj.nil?
      out << "0"
    elsif obj == true
      out << "T"
    elsif obj == false
      out << "F"
    elsif obj.is_a?(Integer)
      out << "i" << obj.to_s << ";"
    elsif obj.is_a?(Float)
      out << "f" << obj.to_s << ";"
    elsif obj.is_a?(Symbol)
      str = obj.to_s
      out << "y" << str.length.to_s << ":" << str
    elsif obj.is_a?(String)
      out << "s" << obj.length.to_s << ":" << obj
    elsif obj.is_a?(Array)
      out << "[" << obj.length.to_s << ";"
      obj.each { |item| _dump(item, out) }
    elsif obj.is_a?(Hash)
      out << "{" << obj.length.to_s << ";"
      obj.each { |key, value| _dump(key, out); _dump(value, out) }
    else
      raise TypeError, "no _dump_data is defined for class #{obj.class}"
    end
  end

  def self.load(data, *_args)
    _load(data.to_s, [0])
  end

  def self.restore(data, *args)
    load(data, *args)
  end

  def self._load(str, pos)
    tag = str[pos[0]]
    pos[0] += 1
    case tag
    when "0" then nil
    when "T" then true
    when "F" then false
    when "i" then _read_to(str, pos, ";").to_i
    when "f" then _read_to(str, pos, ";").to_f
    when "y"
      len = _read_to(str, pos, ":").to_i
      val = str[pos[0], len]
      pos[0] += len
      val.to_sym
    when "s"
      len = _read_to(str, pos, ":").to_i
      val = str[pos[0], len]
      pos[0] += len
      val
    when "["
      count = _read_to(str, pos, ";").to_i
      arr = []
      idx = 0
      while idx < count
        arr << _load(str, pos)
        idx += 1
      end
      arr
    when "{"
      count = _read_to(str, pos, ";").to_i
      hash = {}
      idx = 0
      while idx < count
        key = _load(str, pos)
        value = _load(str, pos)
        hash[key] = value
        idx += 1
      end
      hash
    else
      raise ArgumentError, "marshal data too short or corrupt"
    end
  end

  def self._read_to(str, pos, delim)
    start = pos[0]
    idx = str.index(delim, start)
    raise ArgumentError, "marshal data corrupt" if idx.nil?
    val = str[start...idx]
    pos[0] = idx + 1
    val
  end
end
"#;
        let marshal_path = PathBuf::from(VIRTUAL_ROOT).join("marshal.rb");
        env.def_rb_source_file(marshal_path, marshal.as_bytes().to_vec())?;

        // Real OptionParser: `on` records switch definitions (short `-x`, long
        // `--name`, argument-taking `--name VAL`/`--name=VAL`, optional `[VAL]`,
        // and `--[no-]flag`); `parse!`/`parse`/`order!` walk an argv, invoke the
        // blocks, support `--` end-of-options and bundled short flags, and leave
        // the non-option residue (parse! mutates argv in place). Type coercion
        // and the auto-generated help table are not implemented.
        let optparse = r#"
class OptionParser
  class ParseError < StandardError; end
  class InvalidOption < ParseError; end
  class MissingArgument < ParseError; end
  class InvalidArgument < ParseError; end
  class AmbiguousOption < ParseError; end

  attr_accessor :banner, :program_name

  def initialize(banner = nil, *_rest)
    @banner = banner
    @program_name = "regent"
    @switches = []
    yield self if block_given?
  end

  def on(*args, &block)
    short = nil
    long = nil
    takes_arg = false
    optional_arg = false
    args.each do |arg|
      next unless arg.is_a?(String)
      if arg.start_with?("--")
        body = arg[2..-1]
        if body.start_with?("[no-]")
          long = body[5..-1].split(/[ =\[]/).first
        else
          parts = body.split(/[ =]/, 2)
          long = parts[0]
          if parts.length > 1
            takes_arg = true
            optional_arg = true if parts[1].start_with?("[")
          end
        end
      elsif arg.start_with?("-") && arg.length >= 2
        rest = arg[1..-1]
        short = rest[0]
        takes_arg = true if rest.length > 1
      end
    end
    @switches << {
      "short" => short,
      "long" => long,
      "arg" => takes_arg,
      "optional" => optional_arg,
      "block" => block
    }
    self
  end

  def def_option(*args, &block)
    on(*args, &block)
  end

  def separator(*); end
  def to_s; @banner.to_s; end
  def help; to_s; end
  def order!(argv); parse!(argv); end

  def parse!(argv)
    rest = []
    i = 0
    while i < argv.length
      tok = argv[i]
      if tok == "--"
        i += 1
        while i < argv.length
          rest << argv[i]
          i += 1
        end
        break
      elsif tok.start_with?("--")
        body = tok[2..-1]
        eqpos = body.index("=")
        if eqpos
          name = body[0...eqpos]
          inline = body[(eqpos + 1)..-1]
          has_eq = true
        else
          name = body
          inline = nil
          has_eq = false
        end
        sw = _find_long(name)
        negated = false
        if sw.nil? && name.start_with?("no-")
          sw = _find_long(name[3..-1])
          negated = true unless sw.nil?
        end
        raise InvalidOption, "invalid option: --#{name}" if sw.nil?
        if sw["arg"]
          if has_eq
            value = inline
          elsif sw["optional"]
            value = nil
          else
            i += 1
            value = argv[i]
            raise MissingArgument, "missing argument: --#{name}" if value.nil?
          end
          sw["block"].call(value) if sw["block"]
        else
          sw["block"].call(!negated) if sw["block"]
        end
      elsif tok.start_with?("-") && tok.length >= 2
        chars = tok[1..-1]
        j = 0
        while j < chars.length
          c = chars[j]
          sw = _find_short(c)
          raise InvalidOption, "invalid option: -#{c}" if sw.nil?
          if sw["arg"]
            value = chars[(j + 1)..-1]
            if value.nil? || value.empty?
              if sw["optional"]
                value = nil
              else
                i += 1
                value = argv[i]
                raise MissingArgument, "missing argument: -#{c}" if value.nil?
              end
            end
            sw["block"].call(value) if sw["block"]
            break
          else
            sw["block"].call(true) if sw["block"]
            j += 1
          end
        end
      else
        rest << tok
      end
      i += 1
    end
    argv.clear
    rest.each { |item| argv << item }
    argv
  end

  def parse(argv)
    parse!(argv.dup)
  end

  def _find_long(name)
    @switches.each { |sw| return sw if sw["long"] == name }
    nil
  end

  def _find_short(ch)
    @switches.each { |sw| return sw if sw["short"] == ch }
    nil
  end
end
"#;
        let optparse_path = PathBuf::from(VIRTUAL_ROOT).join("optparse.rb");
        env.def_rb_source_file(optparse_path, optparse.as_bytes().to_vec())?;

        let english = r#"
$ERROR_INFO = nil
$ERROR_POSITION = nil
$FS = $; = nil
$INPUT_RECORD_SEPARATOR = $/ = "\n"
$OUTPUT_RECORD_SEPARATOR = $\\ = nil
$FIELD_SEPARATOR = $, = nil
$OUTPUT_FIELD_SEPARATOR = $, = nil
$PROCESS_ID = $$ = 0
$CHILD_STATUS = $? = nil
$LAST_READ_LINE = $_ = nil
$LAST_MATCH_INFO = $~ = nil
$LAST_PAREN_MATCH = $+ = nil
$PREMATCH = $` = nil
$POSTMATCH = $' = nil
"#;
        let english_path = PathBuf::from(VIRTUAL_ROOT).join("English.rb");
        env.def_rb_source_file(english_path, english.as_bytes().to_vec())?;

        // Artichoke ships no Base64 stdlib module, yet Puppet helpers and specs
        // (e.g. facter ssh helpers) `require 'base64'`. Provide a self-contained
        // pure-Ruby implementation that fully mirrors the canonical base64 gem
        // (RFC-2045 and RFC-4648 method pairs, padding/newline rules, and strict
        // validation). It avoids pack/unpack, tr, ljust, and enumerator chaining,
        // which Artichoke does not fully support.
        let base64 = r#"
module Base64
  VERSION = "0.3.0"

  STANDARD = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/"
  URLSAFE  = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_"

  def self._encode(bin, chars)
    bytes = bin.to_s.bytes
    out = +""
    i = 0
    len = bytes.length
    while i < len
      b0 = bytes[i]
      b1 = bytes[i + 1]
      b2 = bytes[i + 2]
      n = (b0 << 16) | ((b1 || 0) << 8) | (b2 || 0)
      out << chars[(n >> 18) & 63]
      out << chars[(n >> 12) & 63]
      if b1.nil?
        out << "=="
      elsif b2.nil?
        out << chars[(n >> 6) & 63]
        out << "="
      else
        out << chars[(n >> 6) & 63]
        out << chars[n & 63]
      end
      i += 3
    end
    out
  end

  def self._lookup(chars)
    table = {}
    idx = 0
    while idx < chars.length
      table[chars[idx]] = idx
      idx += 1
    end
    table
  end

  # Lenient decode: non-alphabet characters (newlines, '=', and the unused
  # half of the alphabet) are ignored, and padding is not checked.
  def self._decode_lenient(str, chars)
    table = _lookup(chars)
    s = str.to_s
    bits = 0
    bit_count = 0
    out = +""
    i = 0
    while i < s.length
      c = s[i]
      i += 1
      val = table[c]
      next if val.nil?
      bits = (bits << 6) | val
      bit_count += 6
      if bit_count >= 8
        bit_count -= 8
        out << ((bits >> bit_count) & 0xFF).chr
        # Keep only the still-unconsumed low bits so the accumulator never
        # grows past ~12 bits; Artichoke raises on wide bit shifts.
        bits &= (1 << bit_count) - 1
      end
    end
    out
  end

  # Strict decode: length must be a multiple of 4, padding (if any) must be a
  # correct 1 or 2 trailing '=', and every other character must be in the
  # alphabet. Anything else raises ArgumentError, matching unpack1("m0").
  def self._decode_strict(str, chars)
    s = str.to_s
    raise ArgumentError, "invalid base64" if s.length % 4 != 0
    pad = 0
    pad += 1 if s.length >= 1 && s[s.length - 1] == "="
    pad += 1 if s.length >= 2 && s[s.length - 2] == "="
    body = pad > 0 ? s[0, s.length - pad] : s
    table = _lookup(chars)
    bits = 0
    bit_count = 0
    out = +""
    i = 0
    while i < body.length
      c = body[i]
      i += 1
      val = table[c]
      raise ArgumentError, "invalid base64" if val.nil?
      bits = (bits << 6) | val
      bit_count += 6
      if bit_count >= 8
        bit_count -= 8
        out << ((bits >> bit_count) & 0xFF).chr
        bits &= (1 << bit_count) - 1
      end
    end
    out
  end

  def self.encode64(bin)
    encoded = _encode(bin, STANDARD)
    result = +""
    i = 0
    while i < encoded.length
      result << encoded[i, 60]
      result << "\n"
      i += 60
    end
    result
  end

  def self.decode64(str)
    _decode_lenient(str, STANDARD)
  end

  def self.strict_encode64(bin)
    _encode(bin, STANDARD)
  end

  def self.strict_decode64(str)
    _decode_strict(str, STANDARD)
  end

  def self.urlsafe_encode64(bin, padding: true)
    str = _encode(bin, URLSAFE)
    unless padding
      str = str[0, str.length - 2] if str.length >= 2 && str[str.length - 2, 2] == "=="
      str = str[0, str.length - 1] if str.length >= 1 && str[str.length - 1] == "="
    end
    str
  end

  def self.urlsafe_decode64(str)
    # RFC 4648 says nothing about unpadded input but allows excess pad
    # characters to be ignored, so unpadded input is accepted too: pad it up
    # to a multiple of 4 before strict validation.
    s = str.to_s
    if (s.empty? || s[s.length - 1] != "=") && s.length % 4 != 0
      target = ((s.length + 3) / 4) * 4
      s += "=" while s.length < target
    end
    _decode_strict(s, URLSAFE)
  end
end
"#;
        let base64_path = PathBuf::from(VIRTUAL_ROOT).join("base64.rb");
        env.def_rb_source_file(base64_path, base64.as_bytes().to_vec())?;

        // Apply the Hash#merge keyword-binding workaround immediately (not as a
        // require-on-demand virtual file) so every spec and helper sees it.
        env.eval(HASH_MERGE_FIX)?;

        Ok(())
    }

    fn parse_summary(output: &str) -> Summary {
        let mut summary = Summary::default();
        for line in output.lines() {
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            match key {
                "exit_code" => summary.exit_code = value.parse().unwrap_or(1),
                "total" => summary.total = value.parse().unwrap_or(0),
                "failed" => summary.failed = value.parse().unwrap_or(0),
                "pending" => summary.pending = value.parse().unwrap_or(0),
                "passed" => summary.passed = value.parse().unwrap_or(0),
                "stdout" => summary.stdout = value.to_string(),
                "stderr" => summary.stderr = value.replace("\\n", "\n"),
                _ => {}
            }
        }
        summary
    }
}

struct Summary {
    exit_code: i32,
    total: usize,
    failed: usize,
    pending: usize,
    passed: usize,
    stdout: String,
    stderr: String,
}

#[derive(Debug, Clone)]
struct SupportedOs {
    os: String,
    release: String,
}

impl SupportedOs {
    fn new(os: &str, release: &str) -> Self {
        Self {
            os: os.to_string(),
            release: release.to_string(),
        }
    }

    fn osfamily(&self) -> &'static str {
        match self.os.to_lowercase().as_str() {
            "redhat" | "centos" | "rocky" | "almalinux" | "oraclelinux" | "fedora"
            | "scientific" => "RedHat",
            "debian" | "ubuntu" => "Debian",
            "sles" | "suse" | "opensuse" => "Suse",
            "solaris" => "Solaris",
            "freebsd" => "FreeBSD",
            "openbsd" => "OpenBSD",
            "darwin" => "Darwin",
            "windows" => "windows",
            "archlinux" | "arch" => "Archlinux",
            "gentoo" => "Gentoo",
            _ => "RedHat",
        }
    }

    fn key(&self) -> String {
        if self.release.is_empty() {
            format!("{}-x86_64", self.os.to_lowercase())
        } else {
            format!("{}-{}-x86_64", self.os.to_lowercase(), self.release)
        }
    }
}

fn render_supported_os_ruby(entries: &[SupportedOs]) -> String {
    if entries.is_empty() {
        return "{ \"default\" => {} }".to_string();
    }
    let mut buf = String::from("{ ");
    for (i, entry) in entries.iter().enumerate() {
        if i > 0 {
            buf.push_str(", ");
        }
        let key = ruby_string_literal(&entry.key());
        let os = ruby_string_literal(&entry.os.to_lowercase());
        let release = ruby_string_literal(&entry.release);
        let family = ruby_string_literal(entry.osfamily());
        buf.push_str(&format!(
            "{key} => {{ \"os\" => {{ \"family\" => {family}, \"name\" => {os}, \"release\" => {{ \"full\" => {release}, \"major\" => {release} }} }}, \"osfamily\" => {family}, \"operatingsystem\" => {os}, \"operatingsystemrelease\" => {release}, \"operatingsystemmajrelease\" => {release}, \"architecture\" => \"x86_64\", \"kernel\" => {family} }}"
        ));
    }
    buf.push_str(" }");
    buf
}

fn ruby_string_literal(value: &str) -> String {
    let escaped = value.replace('\\', "\\\\").replace('"', "\\\"");
    format!("\"{escaped}\"")
}

impl Default for Summary {
    fn default() -> Self {
        Self {
            exit_code: 1,
            total: 0,
            failed: 0,
            pending: 0,
            passed: 0,
            stdout: String::new(),
            stderr: String::new(),
        }
    }
}
