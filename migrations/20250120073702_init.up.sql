-- Add up migration script here
CREATE SCHEMA chat;

CREATE TABLE chat.users (
    id VARCHAR(64) PRIMARY KEY, -- 使用 SERIAL 自动生成唯一整数 ID
    name VARCHAR(64) NOT NULL, -- 用户名，最大长度 64
    password_hash VARCHAR(255) NOT NULL -- 存储密码的哈希值
);

CREATE TABLE chat.channel (
    id SERIAL PRIMARY KEY,
    name VARCHAR(64) NOT NULL,
    limit_num INT NOT NULL,
    owner_id VARCHAR(64) NOT NULL,
    FOREIGN KEY (owner_id) REFERENCES chat.users(id),
    CONSTRAINT check_limit CHECK (limit_num > 0)
);
