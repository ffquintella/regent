# frozen_string_literal: true

require 'json'

module Regent
  class DependencyResolver
    attr_reader :path, :options

    def initialize(path, options = {})
      @path = path
      @options = options
    end

    def resolve
      metadata = load_metadata
      return metadata if metadata[:success] == false

      dependencies = metadata.fetch('dependencies', [])
      return empty_result if dependencies.empty?

      unless command_available?('puppet')
        return {
          success: false,
          error: 'OpenVox Puppet not found. Install OpenVox Puppet before running Regent tests.',
          help: 'Install instructions: https://openvoxproject.org/downloads/'
        }
      end

      empty_result
    rescue StandardError => e
      { success: false, error: e.message }
    end

    private

    def empty_result
      { success: true, installed: [], skipped: [], errors: [] }
    end

    def load_metadata
      metadata_path = File.join(path, 'metadata.json')
      return { success: false, error: 'metadata.json not found' } unless File.exist?(metadata_path)

      JSON.parse(File.read(metadata_path))
    rescue JSON::ParserError => e
      { success: false, error: "Invalid JSON in metadata.json: #{e.message}" }
    end

    def command_available?(command)
      ENV['PATH'].to_s.split(File::PATH_SEPARATOR).any? do |dir|
        File.executable?(File.join(dir, command))
      end
    end
  end
end
