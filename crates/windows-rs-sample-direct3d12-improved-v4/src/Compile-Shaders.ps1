. .\Activate-FXC.ps1
fxc /T vs_5_0 /E VSMain /Fo shaders_vs.cso shaders.hlsl
fxc /T ps_5_0 /E PSMain /Fo shaders_ps.cso shaders.hlsl
