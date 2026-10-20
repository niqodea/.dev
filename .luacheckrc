-- Neovim runs Lua through LuaJIT
std = 'luajit'
-- Neovim's API is a global, and setting its fields, like vim.g.mapleader, is how it is configured
globals = { 'vim' }
-- The repo sets no line length for any language, and its Lua already has longer lines
max_line_length = false
