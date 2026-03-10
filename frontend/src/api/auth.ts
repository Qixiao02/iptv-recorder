import apiClient from './client';

export interface LoginRequest {
  username: string;
  password: string;
}

export interface LoginResponse {
  token: string;
  user: UserInfo;
}

export interface UserInfo {
  id: string;
  username: string;
  nickname: string | null;
  role: string;
}

export interface ChangePasswordRequest {
  old_password: string;
  new_password: string;
}

export interface UpdateProfileRequest {
  nickname?: string;
}

// 登录
export const login = (data: LoginRequest): Promise<LoginResponse> => {
  return apiClient.post('/auth/login', data).then((res) => res.data);
};

// 获取当前用户信息
export const getCurrentUser = (): Promise<UserInfo> => {
  return apiClient.get('/auth/me').then((res) => res.data);
};

// 修改密码
export const changePassword = (data: ChangePasswordRequest): Promise<void> => {
  return apiClient.post('/auth/password', data).then((res) => res.data);
};

// 更新用户资料
export const updateProfile = (data: UpdateProfileRequest): Promise<UserInfo> => {
  return apiClient.post('/auth/profile', data).then((res) => res.data);
};
