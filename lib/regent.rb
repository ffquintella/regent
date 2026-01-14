# frozen_string_literal: true

require_relative 'regent/version'
require_relative 'regent/config'
require_relative 'regent/generator'
require_relative 'regent/validator'
require_relative 'regent/builder'
require_relative 'regent/tester'

module Regent
  class Error < StandardError; end

  class << self
    attr_accessor :config

    def configure
      self.config ||= Config.new
      yield(config) if block_given?
      config
    end

    def root
      File.expand_path('../..', __dir__)
    end
  end
end
