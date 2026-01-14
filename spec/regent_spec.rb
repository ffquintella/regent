# frozen_string_literal: true

RSpec.describe Regent do
  it 'has a version number' do
    expect(Regent::VERSION).not_to be nil
  end

  it 'has a version in correct format' do
    expect(Regent::VERSION).to match(/^\d+\.\d+\.\d+$/)
  end
end
