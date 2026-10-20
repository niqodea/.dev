vim.api.nvim_create_user_command('GitActiveFiles', function()
    vim.cmd('terminal git active-files')
end, {})

vim.api.nvim_create_user_command('GitRecentFiles', function()
    vim.cmd('terminal git recent-files')
end, {})

vim.api.nvim_create_user_command('GitStatus', function()
    vim.cmd('terminal git status --short')
end, {})

local function run_git(arguments)
    local output = vim.fn.systemlist(vim.list_extend({ 'git' }, arguments))
    if vim.v.shell_error ~= 0 then
        error('git ' .. table.concat(arguments, ' ') .. ' failed: ' .. table.concat(output, ' '))
    end
    return output
end

local function complete_branch(argument)
    local branches = run_git({ 'for-each-ref', '--format=%(refname:short)', 'refs/heads', 'refs/remotes' })
    return vim.tbl_filter(function(branch) return vim.startswith(branch, argument) end, branches)
end

-- Lists, one quickfix entry per hunk, what git commit --all would put on the branch since it left its base
-- That leaves out untracked files, which join the project only once added
local function review()
    local base_branch = run_git({ 'base-branch' })[1]
    local branch_point = run_git({ 'merge-base', base_branch, 'HEAD' })[1]
    local root = run_git({ 'rev-parse', '--show-toplevel' })[1]
    local diff_lines = run_git({ 'diff', '--unified=0', branch_point })
    local items = {}
    local path
    for _, line in ipairs(diff_lines) do
        if vim.startswith(line, '+++ ') then
            -- A deleted file has no b/ path, and no lines left to jump to
            path = line:match('^%+%+%+ b/(.*)')
        end
        local line_number = line:match('^@@ %-%S+ %+(%d+)')
        if line_number ~= nil and path ~= nil then
            -- A hunk that only deletes the first lines starts at line 0
            table.insert(items, { filename = root .. '/' .. path, lnum = math.max(tonumber(line_number), 1), text = line })
        end
    end
    vim.fn.setqflist({}, ' ', { title = 'Review against ' .. base_branch, items = items })
    vim.cmd('copen')
end

local function get_lines_at(commit, path)
    local folder = vim.fn.fnamemodify(path, ':h')
    local name = vim.fn.fnamemodify(path, ':t')
    local listed = run_git({ '-C', folder, 'ls-tree', '--name-only', commit, '--', name })
    -- A file added since that commit has no earlier version
    if #listed == 0 then
        return {}
    end
    return run_git({ '-C', folder, 'show', commit .. ':./' .. name })
end

-- Opens the file as it was where HEAD left its base branch, side by side with the file now
local function diff()
    local base_branch = run_git({ 'base-branch' })[1]
    local branch_point = run_git({ 'merge-base', base_branch, 'HEAD' })[1]
    local lines = get_lines_at(branch_point, vim.fn.expand('%:p'))
    local filetype = vim.bo.filetype
    vim.cmd('diffthis')
    vim.cmd('leftabove vnew')
    vim.api.nvim_buf_set_lines(0, 0, -1, false, lines)
    vim.bo.buftype = 'nofile'
    vim.bo.bufhidden = 'wipe'
    vim.bo.modifiable = false
    vim.bo.filetype = filetype
    vim.cmd('diffthis')
    -- Closing the old version takes the file out of diff mode too
    vim.api.nvim_create_autocmd('BufWipeout', { buffer = 0, command = 'diffoff!' })
    vim.notify('Comparing with where HEAD left ' .. base_branch)
end

vim.keymap.set('n', '<leader>gr', review)
vim.keymap.set('n', '<leader>gd', diff)
vim.api.nvim_create_user_command('GitSetBase', function(args)
    run_git({ 'base-branch', args.args })
end, { nargs = 1, complete = complete_branch })
