-- One column for git signs, always shown so the text does not shift
vim.wo.signcolumn = 'yes:1'

vim.api.nvim_set_hl(0, 'GitSignAdd', { ctermfg = 'DarkGreen' })
vim.api.nvim_set_hl(0, 'GitSignChange', { ctermfg = 'DarkYellow' })
vim.api.nvim_set_hl(0, 'GitSignDelete', { ctermfg = 'DarkRed' })

local signs_namespace = vim.api.nvim_create_namespace('git-signs')

local function add_sign(buf, line, text, hl_group)
    vim.api.nvim_buf_set_extmark(buf, signs_namespace, line - 1, 0, { sign_text = text, sign_hl_group = hl_group })
end

-- Marks the lines where the buffer differs from its file's last commit
local function update_signs(buf)
    vim.api.nvim_buf_clear_namespace(buf, signs_namespace, 0, -1)
    local path = vim.api.nvim_buf_get_name(buf)
    -- Terminals, folders, scratch buffers and unnamed buffers hold no file that git could know
    if vim.bo[buf].buftype ~= '' or path == '' then return end
    local root = vim.fs.root(path, '.git')
    if root == nil then return end
    -- vim.system keeps the warnings git writes to stderr out of the committed text
    local result = vim.system({ 'git', '-C', root, 'show', 'HEAD:' .. vim.fs.relpath(root, path) }, { text = true })
        :wait()
    -- A file missing from the last commit, like a new one, has nothing to compare against, so it gets no signs
    if result.code ~= 0 then return end
    -- Both texts end in a newline, as the buffer's lines give no way to tell a file that lacks one
    local committed_text = result.stdout:sub(-1) == '\n' and result.stdout or result.stdout .. '\n'
    local buffer_text = table.concat(vim.api.nvim_buf_get_lines(buf, 0, -1, false), '\n') .. '\n'
    -- Each hunk is { old start, old count, new start, new count }, like a git diff hunk header
    local hunks = vim.text.diff(committed_text, buffer_text, { result_type = 'indices' })
    for _, hunk in ipairs(hunks) do
        local old_count, first_line, new_count = hunk[2], hunk[3], hunk[4]
        if new_count == 0 then
            -- A deletion's new start is the line above the deleted lines, or 0 when they were the first lines
            add_sign(buf, math.max(first_line, 1), first_line == 0 and '▔' or '▁', 'GitSignDelete')
        else
            local hl_group = old_count == 0 and 'GitSignAdd' or 'GitSignChange'
            for changed_line = first_line, first_line + new_count - 1 do
                add_sign(buf, changed_line, '┃', hl_group)
            end
        end
    end
end

-- Typing updates the signs when insert mode ends, since updating on each key would run git on each key
-- Entering a buffer and regaining focus catch commits made outside the buffer
vim.api.nvim_create_autocmd({ 'TextChanged', 'InsertLeave', 'BufEnter', 'FocusGained' }, {
    callback = function(args) update_signs(args.buf) end,
})

local fzf_lua = require('x.core.fuzzy').fzf_lua

vim.keymap.set('n', '<leader>fg-', function() fzf_lua.git_files() end)
vim.keymap.set('n', '<leader>fg/', function() fzf_lua.live_grep({ cmd = 'git grep --line-number --column' }) end)
vim.keymap.set('n', '<leader>fg*', function() fzf_lua.grep_cword({ cmd = 'git grep --line-number --column' }) end)
vim.keymap.set('v', '<leader>fg*', function() fzf_lua.grep_visual({ cmd = 'git grep --line-number --column' }) end)

-- The capital F searches from the current file's folder
vim.keymap.set('n', '<leader>Fg-', function() fzf_lua.git_files({ cwd = vim.fn.expand('%:p:h') }) end)
vim.keymap.set(
    'n',
    '<leader>Fg/',
    function() fzf_lua.live_grep({ cwd = vim.fn.expand('%:p:h'), cmd = 'git grep --line-number --column' }) end
)
vim.keymap.set(
    'n',
    '<leader>Fg*',
    function() fzf_lua.grep_cword({ cwd = vim.fn.expand('%:p:h'), cmd = 'git grep --line-number --column' }) end
)
vim.keymap.set(
    'v',
    '<leader>Fg*',
    function() fzf_lua.grep_visual({ cwd = vim.fn.expand('%:p:h'), cmd = 'git grep --line-number --column' }) end
)

require('x.modules.git-commands')
