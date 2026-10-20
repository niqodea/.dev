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
-- That leaves out untracked files, since git commit --all does not add them
local function diff_all()
    local base_branch = run_git({ 'base-branch' })[1]
    local branch_point = run_git({ 'merge-base', base_branch, 'HEAD' })[1]
    local root = run_git({ 'rev-parse', '--show-toplevel' })[1]
    -- --no-renames lists a renamed file as deleted and added, so each path is new or exists at the branch point
    -- core.quotePath=false keeps non-ASCII file names unquoted, as the header match reads them
    local diff_lines = run_git({ '-c', 'core.quotePath=false', 'diff', '--unified=0', '--no-renames', branch_point })
    local items = {}
    local path
    -- A changed line can look like a header line, so file names are read only from the header
    local in_header = false
    for _, line in ipairs(diff_lines) do
        if vim.startswith(line, 'diff --git ') then
            in_header = true
        elseif vim.startswith(line, '@@ ') then
            in_header = false
            local line_number = line:match('^@@ %-%S+ %+(%d+)')
            -- A hunk that only deletes the first lines starts at line 0
            table.insert(items, { filename = root .. '/' .. path, lnum = math.max(tonumber(line_number), 1), text = '' })
        elseif in_header then
            -- A deleted file has only its old path, and an added one only its new path
            -- git ends a name that contains a space with a tab, which is not part of the name
            path = line:match('^%-%-%- a/(.-)\t?$') or line:match('^%+%+%+ b/(.-)\t?$') or path
        else
            local item = items[#items]
            -- An added line shows what the branch wrote, so it wins over a removed one
            if item.text == '' or (vim.startswith(line, '+') and not vim.startswith(item.text, '+')) then
                item.text = line
            end
        end
    end
    vim.fn.setqflist({}, ' ', { title = 'Changes since HEAD left ' .. base_branch, items = items })
    vim.cmd('copen')
end

-- The file in a buffer as it was at a commit, read-only and named after the commit
local function get_version_buf(commit, file_buf)
    local path = vim.api.nvim_buf_get_name(file_buf)
    local short_commit = run_git({ 'rev-parse', '--short', commit })[1]
    local buf = vim.fn.bufadd(short_commit .. ':' .. vim.fn.fnamemodify(path, ':.'))
    -- Another tab may have filled this buffer already, and a commit's version never changes
    if vim.b[buf].git_version ~= nil then
        return buf
    end
    -- Found from the path alone, since a file deleted with its folder leaves no folder to run git in
    local root = assert(vim.fs.root(path, '.git'), path .. ' is in no git repository')
    local relative_path = vim.fs.relpath(root, path)
    local listed = run_git({ '-C', root, 'ls-tree', '--name-only', commit, '--', relative_path })
    vim.bo[buf].buftype = 'nofile'
    vim.bo[buf].bufhidden = 'wipe'
    vim.fn.bufload(buf)
    -- A file added since that commit has no version there
    if #listed > 0 then
        local lines = run_git({ '-C', root, 'show', commit .. ':' .. relative_path })
        vim.api.nvim_buf_set_lines(buf, 0, -1, false, lines)
    end
    vim.bo[buf].modifiable = false
    vim.bo[buf].filetype = vim.bo[file_buf].filetype
    -- The file whose old version this buffer holds
    vim.b[buf].git_version = file_buf
    return buf
end

-- Opens the file as it was where HEAD left its base branch in a split to the right, or closes that split
-- A tab holds one comparison, since Vim diffs all of a tab's diff windows together
local function toggle_diff()
    -- Pressed in the old-version pane, toggle_diff works on the file the pane is an old version of
    local file_buf = vim.b.git_version or vim.api.nvim_get_current_buf()
    local version_window = vim.iter(vim.api.nvim_tabpage_list_wins(0)):find(function(window)
        return vim.b[vim.api.nvim_win_get_buf(window)].git_version ~= nil
    end)
    if version_window ~= nil then
        -- Read before closing, since closing wipes the buffer
        local compared_buf = vim.b[vim.api.nvim_win_get_buf(version_window)].git_version
        local compared_window = vim.w[version_window].git_file_window
        vim.api.nvim_win_close(version_window, false)
        -- Vim restores a file's last 'diff' in a window, so the compared file is shown there briefly for diffoff
        if vim.api.nvim_win_is_valid(compared_window) and vim.api.nvim_buf_is_valid(compared_buf)
            and vim.api.nvim_win_get_buf(compared_window) ~= compared_buf then
            vim.api.nvim_win_call(compared_window, function()
                local view = vim.fn.winsaveview()
                local shown_buf = vim.api.nvim_get_current_buf()
                vim.cmd('noautocmd keepalt keepjumps buffer ' .. compared_buf)
                vim.cmd('diffoff')
                vim.cmd('noautocmd keepalt keepjumps buffer ' .. shown_buf)
                vim.fn.winrestview(view)
            end)
        end
        if compared_buf == file_buf then
            return
        end
        vim.notify('Replaced the diff of ' .. vim.fn.bufname(compared_buf))
    end
    local base_branch = run_git({ 'base-branch' })[1]
    local branch_point = run_git({ 'merge-base', base_branch, 'HEAD' })[1]
    local version_buf = get_version_buf(branch_point, file_buf)
    local file_window = vim.api.nvim_get_current_win()
    vim.cmd('diffthis')
    local new_version_window = vim.api.nvim_open_win(version_buf, false, { split = 'right' })
    vim.api.nvim_win_call(new_version_window, function() vim.cmd('diffthis') end)
    vim.w[new_version_window].git_file_window = file_window
end

vim.keymap.set('n', '<leader>gd', toggle_diff)
vim.keymap.set('n', '<leader>gD', diff_all)
vim.api.nvim_create_user_command('GitSetBase', function(args)
    run_git({ 'base-branch', args.args })
end, { nargs = 1, complete = complete_branch })
