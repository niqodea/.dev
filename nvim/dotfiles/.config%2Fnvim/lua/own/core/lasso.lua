local M = {}

vim.cmd('packadd lasso.nvim')
M.lasso = require('lasso')
M.lasso.setup{marks_tracker_path = require('own.core.utils').get_atdir() .. '/lasso-marks-tracker'}

vim.keymap.set('n', vim.g.mapleader..'m', function() M.lasso.mark_file() end)
vim.keymap.set('n', vim.g.mapleader..'M', function() M.lasso.open_marks_tracker() end)
vim.keymap.set('n', vim.g.mapleader..'<F1>', function() M.lasso.open_marked_file(1) end)
vim.keymap.set('n', vim.g.mapleader..'<F2>', function() M.lasso.open_marked_file(2) end)
vim.keymap.set('n', vim.g.mapleader..'<F3>', function() M.lasso.open_marked_file(3) end)
vim.keymap.set('n', vim.g.mapleader..'<F4>', function() M.lasso.open_marked_file(4) end)

return M
