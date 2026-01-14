# frozen_string_literal: true

module Regent
  class Validator
    attr_reader :path, :options

    def initialize(path, options = {})
      @path = path
      @options = options
    end

    def validate_syntax
      results = {
        success: true,
        errors: [],
        warnings: []
      }

      validate_puppet_files(results)
      validate_json_files(results)
      validate_metadata_syntax(results)

      results[:success] = results[:errors].empty?
      results
    end

    def validate_metadata
      metadata_path = File.join(path, 'metadata.json')
      
      unless File.exist?(metadata_path)
        return { success: false, error: 'metadata.json not found' }
      end

      begin
        metadata = JSON.parse(File.read(metadata_path))
        errors = []

        errors << 'Missing name field' unless metadata['name']
        errors << 'Missing version field' unless metadata['version']
        errors << 'Missing author field' unless metadata['author']
        errors << 'Missing license field' unless metadata['license']

        if errors.any?
          { success: false, errors: errors }
        else
          { success: true, metadata: metadata }
        end
      rescue JSON::ParserError => e
        { success: false, error: "Invalid JSON in metadata.json: #{e.message}" }
      end
    end

    private

    def validate_metadata_syntax(results)
      metadata_path = File.join(path, 'metadata.json')
      
      unless File.exist?(metadata_path)
        results[:errors] << 'metadata.json not found'
        return
      end

      begin
        metadata = JSON.parse(File.read(metadata_path))
        
        results[:errors] << 'Missing name field' unless metadata['name']
        results[:errors] << 'Missing version field' unless metadata['version']
        results[:errors] << 'Missing author field' unless metadata['author']
        results[:errors] << 'Missing license field' unless metadata['license']
      rescue JSON::ParserError => e
        results[:errors] << "Invalid JSON in metadata.json: #{e.message}"
      end
    end

    def validate_puppet_files(results)
      puppet_files = Dir.glob(File.join(path, '**', '*.pp'))
      
      puppet_files.each do |file|
        content = File.read(file)
        
        # Basic syntax checks
        check_puppet_syntax(file, content, results)
      end
    end

    def validate_json_files(results)
      json_files = Dir.glob(File.join(path, '**', '*.json'))
      
      json_files.each do |file|
        begin
          JSON.parse(File.read(file))
        rescue JSON::ParserError => e
          results[:errors] << "Invalid JSON in #{file}: #{e.message}"
        end
      end
    end

    def check_puppet_syntax(file, content, results)
      # Check for common syntax issues
      lines = content.split("\n")
      
      lines.each_with_index do |line, index|
        line_num = index + 1
        
        # Check for trailing whitespace
        if line =~ /\s+$/
          results[:warnings] << "#{file}:#{line_num}: Trailing whitespace"
        end
        
        # Check for tabs
        if line.include?("\t")
          results[:warnings] << "#{file}:#{line_num}: Tab character found, use spaces"
        end
      end
    end
  end
end
