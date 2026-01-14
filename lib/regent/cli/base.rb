# frozen_string_literal: true

require 'thor'
require 'colorize'

module Regent
  module CLI
    class Base < Thor
      class_option :verbose, type: :boolean, aliases: '-v', desc: 'Enable verbose output'

      desc 'version', 'Show Regent version'
      def version
        puts "Regent version #{Regent::VERSION}"
      end

      desc 'new MODULE_NAME', 'Generate a new Puppet module'
      method_option :author, type: :string, aliases: '-a', desc: 'Module author name'
      method_option :license, type: :string, aliases: '-l', default: 'Apache-2.0', desc: 'Module license'
      method_option :summary, type: :string, aliases: '-s', desc: 'Module summary'
      def new(module_name)
        puts "Creating new module: #{module_name}".green
        
        generator = Regent::Generator.new(module_name, options)
        result = generator.generate_module

        if result[:success]
          puts "✓ #{result[:message]}".green
          puts "  Module path: #{result[:path]}".light_blue
        else
          puts "✗ Error: #{result[:error]}".red
          exit 1
        end
      end

      desc 'generate TYPE NAME', 'Generate module components (class, task, plan)'
      method_option :module, type: :string, aliases: '-m', desc: 'Target module name'
      def generate(type, name)
        module_name = options[:module] || detect_module_name
        
        unless module_name
          puts '✗ Error: Could not detect module. Use --module option or run from module directory'.red
          exit 1
        end

        generator = Regent::Generator.new(module_name)
        
        case type
        when 'class'
          result = generator.generate_class(name)
        when 'task'
          result = generator.generate_task(name)
        else
          puts "✗ Error: Unknown type '#{type}'. Valid types: class, task".red
          exit 1
        end

        if result[:success]
          puts "✓ #{result[:message]}".green
        else
          puts "✗ Error: #{result[:error]}".red
          exit 1
        end
      end

      desc 'validate [PATH]', 'Validate module syntax and structure'
      def validate(path = '.')
        puts "Validating module at: #{path}".yellow
        
        validator = Regent::Validator.new(path, options)
        result = validator.validate_syntax

        if result[:success]
          puts '✓ Validation passed'.green
          
          if result[:warnings].any?
            puts "\nWarnings:".yellow
            result[:warnings].each { |warning| puts "  - #{warning}".yellow }
          end
        else
          puts '✗ Validation failed'.red
          
          if result[:errors].any?
            puts "\nErrors:".red
            result[:errors].each { |error| puts "  - #{error}".red }
          end
          
          exit 1
        end
      end

      desc 'build [PATH]', 'Build and package the module'
      def build(path = '.')
        puts "Building module at: #{path}".yellow
        
        builder = Regent::Builder.new(path, options)
        result = builder.build

        if result[:success]
          puts "✓ #{result[:message]}".green
          puts "  Package: #{result[:package_path]}".light_blue
        else
          puts "✗ Error: #{result[:error]}".red
          exit 1
        end
      end

      desc 'test [PATH]', 'Run module tests'
      method_option :type, type: :string, aliases: '-t', desc: 'Test type (unit, integration, all)'
      def test(path = '.')
        test_type = options[:type] || 'all'
        puts "Running #{test_type} tests for module at: #{path}".yellow
        
        tester = Regent::Tester.new(path, options)
        result = tester.run_tests

        if result[:success]
          puts '✓ Tests passed'.green
          puts "  Passed: #{result[:passed]}, Failed: #{result[:failed]}, Skipped: #{result[:skipped]}".light_blue
        else
          if result[:error]
            puts "✗ Error: #{result[:error]}".red
          else
            puts '✗ Tests failed'.red
            puts "  Passed: #{result[:passed]}, Failed: #{result[:failed]}, Skipped: #{result[:skipped]}".light_blue
          end
          exit 1
        end
      end

      private

      def detect_module_name
        metadata_path = File.join(Dir.pwd, 'metadata.json')
        return nil unless File.exist?(metadata_path)

        metadata = JSON.parse(File.read(metadata_path))
        metadata['name']&.split('-')&.last
      rescue StandardError
        nil
      end
    end
  end
end
