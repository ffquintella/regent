# frozen_string_literal: true
# A Puppet plan template
# Documentation: https://puppet.com/docs/bolt/latest/writing_plans.html
#
# This plan demonstrates basic Puppet Bolt functionality.
# Customize the targets, tasks, and commands as needed.

plan <%= module_name %>::<%= class_name %>(
  TargetSpec $targets = 'all',
  Optional[String] $message = undef,
) {
  # Run a simple command on all targets
  run_command('id', $targets)

  # Optional: Display a message
  if $message {
    out::message($message)
  }
}
