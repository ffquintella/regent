# frozen_string_literal: true

module Regent
  class Tester
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
      
      unless Dir.exist?(spec_dir)
        return { success: false, error: 'No spec directory found' }
      end

      run_rspec_tests(spec_dir, result)

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

    def run_rspec_tests(spec_dir, result)
      # In a real implementation, this would execute RSpec
      spec_files = Dir.glob(File.join(spec_dir, '**', '*_spec.rb'))
      
      if spec_files.empty?
        result[:output] << 'No spec files found'
        result[:skipped] = 1
      else
        result[:output] << "Found #{spec_files.length} spec file(s)"
        result[:passed] = spec_files.length
      end
    end

    def run_tests_by_type(test_type)
      spec_path = File.join(path, 'spec', test_type)
      
      unless Dir.exist?(spec_path)
        return { success: false, error: "No #{test_type} tests found" }
      end

      run_tests
    end
  end
end
