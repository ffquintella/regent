This directory should contain the base Ruby gems bundled with Regent for offline test runs.

Expected layout:
- mirror a Bundler path (e.g., ruby/3.2.0/gems, ruby/3.2.0/specifications)
- include all gems required for Artichoke-based tests

You can override this path by setting REGENT_BUNDLED_GEMS.
