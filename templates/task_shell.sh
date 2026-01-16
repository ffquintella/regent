#!/bin/bash
# A Puppet task template written in Bash
# Tasks are scripts that can be run on target systems via Puppet
# Learn more at: https://puppet.com/docs/bolt/latest/writing_tasks.html

set -e

# Exit on error and enable error reporting
trap 'echo \"Error on line $LINENO\" >&2' ERR

# Color codes for output
RED='\\033[0;31m'
GREEN='\\033[0;32m'
YELLOW='\\033[1;33m'
NC='\\033[0m' # No Color

# Main task function
main() {
    local status=\"success\"
    local message=\"Task executed successfully\"
    local timestamp=$(date -u +\"%Y-%m-%dT%H:%M:%SZ\")
    
    # Output result in JSON format
    cat <<EOF
{
  \"status\": \"${status}\",
  \"message\": \"${message}\",
  \"timestamp\": \"${timestamp}\"
}
EOF
    
    return 0
}

# Execute main function
main \"$@\"
