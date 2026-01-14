# frozen_string_literal: true

require 'spec_helper'

RSpec.describe Regent::Config do
  let(:config) { described_class.new }

  describe '#initialize' do
    it 'sets default test framework to rspec' do
      expect(config.test_framework).to eq('rspec')
    end

    it 'sets default syntax checker to puppet-lint' do
      expect(config.syntax_checker).to eq('puppet-lint')
    end

    it 'sets default output format to standard' do
      expect(config.output_format).to eq('standard')
    end
  end

  describe '#validate!' do
    it 'returns true for valid configuration' do
      expect(config.validate!).to be true
    end

    it 'raises error for invalid test framework' do
      config.test_framework = 'invalid'
      expect { config.validate! }.to raise_error(Regent::Error, 'Invalid test framework')
    end

    it 'raises error for invalid syntax checker' do
      config.syntax_checker = 'invalid'
      expect { config.validate! }.to raise_error(Regent::Error, 'Invalid syntax checker')
    end
  end
end
