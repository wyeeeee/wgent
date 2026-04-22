{#
  Available variables:
  - tool_name : &str — Tool name
  - error     : &str — Error message
#}
Tool `{{ tool_name }}` execution failed with the following error:

{{ error }}

Analyze the error and decide whether to retry or try an alternative approach.
