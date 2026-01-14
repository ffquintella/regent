# frozen_string_literal: true

module Regent
  class Config
    attr_accessor :module_template_dir, :test_framework, :syntax_checker, :output_format

    def initialize
      @module_template_dir = File.join(Regent.root, 'templates', 'module')
      @test_framework = 'rspec'
      @syntax_checker = 'puppet-lint'
      @output_format = 'standard'
    end

    def validate!
      raise Error, 'Invalid test framework' unless valid_test_framework?
      raise Error, 'Invalid syntax checker' unless valid_syntax_checker?
      
      true
    end

    private

    def valid_test_framework?
      %w[rspec minitest].include?(@test_framework)
    end

    def valid_syntax_checker?
      %w[puppet-lint rubocop].include?(@syntax_checker)
    end
  end
end
