grpcurl -plaintext -import-path ./proto -proto echo.proto -d '{}' '[::1]:7878' echo.Service/GetUsers
