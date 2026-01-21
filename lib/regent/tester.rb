# frozen_string_literal: true

require 'open3'

module Regent
  class Tester
    ALL_SPEC_DIRS = %w[
      aliases
      classes
      defines
      functions
      hosts
      integration
      plans
      tasks
      type_aliases
      types
      unit
    ].freeze

    attr_reader :path, :options

    def initialize(path, options = {})
      @path = path
      @options = options
    end

    def run_tests
      result = {
        success: true,
        passed: 0,
        failed: 0,
        skipped: 0,
        output: []
      }

      spec_dir = File.join(path, 'spec')
      return { success: false, error: 'No spec directory found' } unless Dir.exist?(spec_dir)

      test_type = options[:type] || 'all'
      pattern = spec_pattern_for(test_type)
      return { success: false, error: "Unknown test type '#{test_type}'" } unless pattern

      spec_files = Dir.glob(File.join(path, pattern))
      if spec_files.empty?
        return { success: false, error: "No #{test_type} spec files found" }
      end

      run_rspec_tests(pattern, result)

      result[:success] = result[:failed].zero?
      result
    rescue StandardError => e
      { success: false, error: e.message }
    end

    def run_unit_tests
      run_tests_by_type('unit')
    end

    def run_integration_tests
      run_tests_by_type('integration')
    end

    private

    def run_rspec_tests(pattern, result)
      command = rspec_command + ['--pattern', pattern, '--format', 'progress']
      env = {}
      if command.first == 'bundle'
        env['BUNDLE_GEMFILE'] = File.join(path, 'Gemfile')
        env['BUNDLE_PATH'] = File.join(path, 'vendor', 'bundle')
        version = bundler_version
        env['BUNDLER_VERSION'] = version if version
        env['GEM_HOME'] = nil
        env['GEM_PATH'] = nil
      else
        env['GEM_HOME'] = ENV['GEM_HOME'] || Gem.dir
        env['GEM_PATH'] = ENV['GEM_PATH'] || Gem.path.join(File::PATH_SEPARATOR)
      end
      stdout, stderr, status = Open3.capture3(env, *command, chdir: path)
      output = [stdout, stderr].join

      if command.first == 'bundle' && bundle_install_needed?(status, output)
        install_env = env.merge('BUNDLE_FORCE_RUBY_PLATFORM' => 'true')
        install_stdout, install_stderr, install_status = Open3.capture3(install_env, 'bundle', 'install', chdir: path)
        install_output = [install_stdout, install_stderr].join

        unless install_status.success?
          result[:success] = false
          result[:error] = install_output.strip
          result[:output] = install_output.lines.map(&:chomp)
          return
        end

        stdout, stderr, status = Open3.capture3(env, *command, chdir: path)
        output = [stdout, stderr].join
      end

      result[:output] = output.lines.map(&:chomp)
      summary = parse_rspec_summary(output) || parse_rspec_errors(output)

      if summary
        result[:passed] = summary[:passed]
        result[:failed] = summary[:failed]
        result[:skipped] = summary[:skipped]
      else
        result[:failed] = status.success? ? 0 : 1
        result[:error] = output.strip unless status.success?
      end
    rescue Errno::ENOENT
      result[:success] = false
      result[:error] = 'RSpec is not available in the current environment'
    end

    def run_tests_by_type(test_type)
      @options = options.merge(type: test_type)
      run_tests
    end

    def spec_pattern_for(test_type)
      case test_type
      when 'all', 'unit'
        "spec/{#{ALL_SPEC_DIRS.join(',')}}/**/*_spec.rb"
      when 'integration'
        "spec/integration/**/*_spec.rb"
      else
        nil
      end
    end

    def rspec_command
      if File.exist?(File.join(path, 'Gemfile')) && command_available?('bundle')
        ['bundle', 'exec', 'rspec']
      elsif rspec_available_in_rubygems?
        [Gem.bin_path('rspec-core', 'rspec')]
      else
        ['rspec']
      end
    end

    def rspec_available_in_rubygems?
      Gem::Specification.find_all_by_name('rspec-core').any?
    rescue StandardError
      false
    end

    def command_available?(command)
      ENV['PATH'].to_s.split(File::PATH_SEPARATOR).any? do |dir|
        File.executable?(File.join(dir, command))
      end
    end

    def bundler_version
      Gem::Specification.find_by_name('bundler').version.to_s
    rescue StandardError, LoadError
      nil
    end

    def bundle_install_needed?(status, output)
      return false if status.success?

      output.include?('command not found: rspec') ||
        output.include?('Install missing gem executables') ||
        output.include?('You must use Bundler') ||
        output.include?('bundle install') ||
        output.include?('Bundler::GemNotFound') ||
        output.include?('Could not find') ||
        output.include?('missing extensions')
    end

    def parse_rspec_summary(output)
      summary_line = output.lines.reverse.find { |line| line =~ /\d+\s+examples?,\s+\d+\s+failures?/ }
      return nil unless summary_line

      match = summary_line.match(/(\d+)\s+examples?,\s+(\d+)\s+failures?(?:,\s+(\d+)\s+pending)?/)
      return nil unless match

      examples = match[1].to_i
      failures = match[2].to_i
      pending = match[3].to_i
      passed = examples - failures - pending

      { passed: passed, failed: failures, skipped: pending }
    end

    def parse_rspec_errors(output)
      error_line = output.lines.reverse.find { |line| line =~ /errors occurred outside of examples/ }
      return nil unless error_line

      match = error_line.match(/(\d+)\s+errors occurred outside of examples/)
      return nil unless match

      { passed: 0, failed: match[1].to_i, skipped: 0 }
    end
  end
end
