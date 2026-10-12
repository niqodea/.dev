alias ,gpg-create-key='gpg --quick-generate-key "$USER" rsa3072 encr,sign 0'
alias ,gpg-decrypt='echo Success | gpg -e -r "$USER" | gpg -d --pinentry-mode loopback'
alias ,pass-init='pass init "$USER"'
