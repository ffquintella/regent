#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
A Puppet task template written in Python.
Tasks are scripts that can be run on target systems via Puppet.
Learn more at: https://puppet.com/docs/bolt/latest/writing_tasks.html
"""

import json
import sys
from datetime import datetime


def run_task(params):
    """
    Execute the task with given parameters.
    
    Args:
        params (dict): Task parameters passed from Puppet Bolt
        
    Returns:
        dict: Task result with status and output
    """
    try:
        return {
            'status': 'success',
            'message': 'Task executed successfully',
            'timestamp': datetime.now().isoformat(),
            'params': params
        }
    except Exception as e:
        return {
            'status': 'error',
            'error_message': str(e),
            'type': type(e).__name__
        }


if __name__ == '__main__':
    # Read parameters from stdin (JSON format)
    try:
        params = json.loads(sys.stdin.read())
    except json.JSONDecodeError:
        params = {}
    
    # Execute task and output result as JSON
    result = run_task(params)
    print(json.dumps(result))
