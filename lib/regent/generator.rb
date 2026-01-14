# frozen_string_literal: true

require 'fileutils'

module Regent
  class Generator
    attr_reader :name, :options, :base_path

    def initialize(name, options = {})
      @name = name
      @options = options
      @base_path = options[:base_path] || Dir.pwd
    end

    def generate_module
      validate_name!
      create_module_structure
      generate_module_files
      
      { success: true, path: module_path, message: "Module '#{name}' created successfully" }
    rescue StandardError => e
      { success: false, error: e.message }
    end

    def generate_class(class_name)
      validate_name!
      class_path = File.join(module_path, 'manifests', "#{class_name}.pp")
      
      create_file(class_path, class_template(class_name))
      
      { success: true, path: class_path, message: "Class '#{class_name}' created successfully" }
    rescue StandardError => e
      { success: false, error: e.message }
    end

    def generate_task(task_name)
      validate_name!
      task_path = File.join(module_path, 'tasks', "#{task_name}.json")
      script_path = File.join(module_path, 'tasks', "#{task_name}.sh")
      
      create_file(task_path, task_metadata_template(task_name))
      create_file(script_path, task_script_template(task_name))
      
      { success: true, path: task_path, message: "Task '#{task_name}' created successfully" }
    rescue StandardError => e
      { success: false, error: e.message }
    end

    private

    def validate_name!
      raise Error, 'Name cannot be empty' if name.nil? || name.empty?
      raise Error, 'Name must be alphanumeric with underscores' unless name.match?(/^[a-z][a-z0-9_]*$/)
    end

    def module_path
      # If we're already inside the module (metadata.json exists in base_path)
      if File.exist?(File.join(base_path, 'metadata.json'))
        @module_path ||= base_path
      else
        # Otherwise, create module directory under base_path
        @module_path ||= File.join(base_path, name)
      end
    end

    def create_module_structure
      dirs = %w[manifests files templates tasks plans lib spec]
      dirs.each do |dir|
        FileUtils.mkdir_p(File.join(module_path, dir))
      end
    end

    def generate_module_files
      create_file(File.join(module_path, 'metadata.json'), metadata_template)
      create_file(File.join(module_path, 'README.md'), readme_template)
      create_file(File.join(module_path, 'manifests', 'init.pp'), init_manifest_template)
      create_file(File.join(module_path, 'spec', 'spec_helper.rb'), spec_helper_template)
    end

    def create_file(path, content)
      FileUtils.mkdir_p(File.dirname(path))
      File.write(path, content)
    end

    def metadata_template
      <<~JSON
        {
          "name": "#{options[:author] || 'author'}-#{name}",
          "version": "0.1.0",
          "author": "#{options[:author] || 'author'}",
          "summary": "#{options[:summary] || "A Puppet module for #{name}"}",
          "license": "#{options[:license] || 'Apache-2.0'}",
          "source": "#{options[:source] || ''}",
          "dependencies": [],
          "operatingsystem_support": [
            {
              "operatingsystem": "RedHat",
              "operatingsystemrelease": ["7", "8"]
            },
            {
              "operatingsystem": "Ubuntu",
              "operatingsystemrelease": ["18.04", "20.04"]
            }
          ]
        }
      JSON
    end

    def readme_template
      <<~README
        # #{name}

        #### Table of Contents

        1. [Description](#description)
        2. [Setup](#setup)
        3. [Usage](#usage)
        4. [Reference](#reference)
        5. [Limitations](#limitations)
        6. [Development](#development)

        ## Description

        #{options[:description] || "This module manages #{name}"}

        ## Setup

        ### What #{name} affects

        * Package installation
        * Configuration files
        * Service management

        ### Beginning with #{name}

        Include the main class:

        ```puppet
        include #{name}
        ```

        ## Usage

        Basic usage:

        ```puppet
        class { '#{name}':
          ensure => present,
        }
        ```

        ## Reference

        See REFERENCE.md for detailed reference documentation.

        ## Limitations

        This module has been tested on:
        * RedHat/CentOS 7, 8
        * Ubuntu 18.04, 20.04

        ## Development

        Contributions are welcome! Please submit pull requests or issues.
      README
    end

    def init_manifest_template
      <<~PUPPET
        # Class: #{name}
        # ===========================
        #
        # Main class for #{name} module
        #
        # Parameters
        # ----------
        #
        # * `ensure`
        # Whether the resource should be present or absent. Default: present
        #
        # Examples
        # --------
        #
        # @example
        #    class { '#{name}':
        #      ensure => present,
        #    }
        #
        class #{name} (
          String $ensure = 'present',
        ) {
          # Main class implementation
        }
      PUPPET
    end

    def spec_helper_template
      <<~RUBY
        # frozen_string_literal: true

        require 'rspec'
        require 'puppet'

        RSpec.configure do |config|
          config.expect_with :rspec do |expectations|
            expectations.include_chain_clauses_in_custom_matcher_descriptions = true
          end

          config.mock_with :rspec do |mocks|
            mocks.verify_partial_doubles = true
          end

          config.shared_context_metadata_behavior = :apply_to_host_groups
        end
      RUBY
    end

    def class_template(class_name)
      <<~PUPPET
        # Class: #{name}::#{class_name}
        # ===========================
        #
        # Manages #{class_name} for #{name}
        #
        class #{name}::#{class_name} (
          String $ensure = 'present',
        ) {
          # Class implementation
        }
      PUPPET
    end

    def task_metadata_template(task_name)
      <<~JSON
        {
          "description": "#{task_name} task for #{name}",
          "parameters": {
            "target": {
              "description": "Target parameter",
              "type": "String"
            }
          }
        }
      JSON
    end

    def task_script_template(task_name)
      <<~BASH
        #!/bin/bash
        # Task: #{task_name}
        # Module: #{name}

        # Parse input parameters
        target="$PT_target"

        echo "Running #{task_name} task"
        echo "Target: $target"

        # Task implementation goes here

        exit 0
      BASH
    end
  end
end
