# frozen_string_literal: true

require 'spec_helper'
require 'tmpdir'
require 'fileutils'

RSpec.describe Regent::Generator do
  let(:module_name) { 'testmodule' }
  let(:options) { { author: 'Test Author', license: 'MIT' } }
  let(:generator) { described_class.new(module_name, options) }
  let(:temp_dir) { Dir.mktmpdir }

  before do
    allow(Dir).to receive(:pwd).and_return(temp_dir)
  end

  after do
    FileUtils.rm_rf(temp_dir)
  end

  describe '#generate_module' do
    it 'creates module successfully' do
      result = generator.generate_module
      expect(result[:success]).to be true
      expect(result[:message]).to include('created successfully')
    end

    it 'creates module directory structure' do
      generator.generate_module
      module_path = File.join(temp_dir, module_name)
      
      expect(Dir.exist?(module_path)).to be true
      expect(Dir.exist?(File.join(module_path, 'manifests'))).to be true
      expect(Dir.exist?(File.join(module_path, 'spec'))).to be true
      expect(Dir.exist?(File.join(module_path, 'tasks'))).to be true
    end

    it 'creates metadata.json' do
      generator.generate_module
      metadata_path = File.join(temp_dir, module_name, 'metadata.json')
      
      expect(File.exist?(metadata_path)).to be true
      metadata = JSON.parse(File.read(metadata_path))
      expect(metadata['name']).to include(module_name)
    end

    it 'creates init.pp manifest' do
      generator.generate_module
      init_path = File.join(temp_dir, module_name, 'manifests', 'init.pp')
      
      expect(File.exist?(init_path)).to be true
      content = File.read(init_path)
      expect(content).to include("class #{module_name}")
    end
  end

  describe '#generate_class' do
    let(:class_name) { 'myclass' }

    before do
      generator.generate_module
    end

    it 'creates class file successfully' do
      result = generator.generate_class(class_name)
      expect(result[:success]).to be true
    end

    it 'creates class manifest file' do
      generator.generate_class(class_name)
      class_path = File.join(temp_dir, module_name, 'manifests', "#{class_name}.pp")
      
      expect(File.exist?(class_path)).to be true
      content = File.read(class_path)
      expect(content).to include("class #{module_name}::#{class_name}")
    end
  end

  describe '#generate_task' do
    let(:task_name) { 'mytask' }

    before do
      generator.generate_module
    end

    it 'creates task files successfully' do
      result = generator.generate_task(task_name)
      expect(result[:success]).to be true
    end

    it 'creates task metadata and script files' do
      generator.generate_task(task_name)
      task_json = File.join(temp_dir, module_name, 'tasks', "#{task_name}.json")
      task_script = File.join(temp_dir, module_name, 'tasks', "#{task_name}.sh")
      
      expect(File.exist?(task_json)).to be true
      expect(File.exist?(task_script)).to be true
    end
  end

  describe 'name validation' do
    it 'rejects empty name' do
      generator = described_class.new('', options)
      result = generator.generate_module
      expect(result[:success]).to be false
    end

    it 'rejects name with invalid characters' do
      generator = described_class.new('Invalid-Name', options)
      result = generator.generate_module
      expect(result[:success]).to be false
    end

    it 'accepts valid name' do
      generator = described_class.new('valid_name', options)
      result = generator.generate_module
      expect(result[:success]).to be true
    end
  end
end
