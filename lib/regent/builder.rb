# frozen_string_literal: true

require 'json'

module Regent
  class Builder
    attr_reader :path, :options

    def initialize(path, options = {})
      @path = path
      @options = options
    end

    def build
      validate_module_structure
      
      result = {
        success: true,
        package_path: nil,
        message: ''
      }

      metadata = load_metadata
      return { success: false, error: 'Failed to load metadata' } unless metadata

      package_name = "#{metadata['name']}-#{metadata['version']}.tar.gz"
      package_path = File.join(path, 'pkg', package_name)

      create_package(package_path)

      result[:package_path] = package_path
      result[:message] = "Module packaged successfully: #{package_name}"
      result
    rescue StandardError => e
      { success: false, error: e.message }
    end

    private

    def validate_module_structure
      required_files = ['metadata.json']
      required_dirs = ['manifests']

      required_files.each do |file|
        raise Error, "Missing required file: #{file}" unless File.exist?(File.join(path, file))
      end

      required_dirs.each do |dir|
        raise Error, "Missing required directory: #{dir}" unless Dir.exist?(File.join(path, dir))
      end
    end

    def load_metadata
      metadata_path = File.join(path, 'metadata.json')
      JSON.parse(File.read(metadata_path))
    rescue StandardError => e
      nil
    end

    def create_package(output_path)
      FileUtils.mkdir_p(File.dirname(output_path))
      
      # Create a simple tar.gz package
      # In a real implementation, this would create a proper Puppet module package
      Dir.chdir(File.dirname(path)) do
        module_name = File.basename(path)
        system("tar -czf #{output_path} #{module_name}")
      end

      raise Error, 'Failed to create package' unless File.exist?(output_path)
    end
  end
end
