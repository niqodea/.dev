-- Neovim runs Lua through LuaJIT
std = 'luajit'
-- Neovim's API is a global, and setting its fields, like vim.g.mapleader, is how it is configured
globals = { 'vim' }
-- StyLua wraps code at its column width, but it cannot break a long string or comment
max_line_length = false
