// In shaders.hlsl

// Access root constants directly by register
// register(b0) corresponds to ShaderRegister: 0 in the root signature
float window_width : register(b0);
// register(b1) corresponds to ShaderRegister: 1 in the root signature
float current_time : register(b1);


struct PSInput
{
    float4 position : SV_POSITION;
    float4 color : COLOR;
};

PSInput VSMain(float4 position : POSITION, float4 color : COLOR)
{
    PSInput result;
    result.position = position;
    result.color = color;
    return result;
}

float4 PSMain(PSInput input) : SV_TARGET
{
    // Use the screen-space x-coordinate and normalize it by window width
    float x_screen = input.position.x;
    float x_normalized = x_screen / window_width;

    // Apply a sine wave function to x_normalized, offset by time
    float frequency = 5.0;
    float speed = 5.0;
    float alpha = (sin((x_normalized - current_time * speed) * frequency * 3.14159) * 0.5) + 0.5;

    // Clamp alpha to the 0-1 range.
    alpha = clamp(alpha, 0.0, 1.0);

    // Use the original vertex color and apply the calculated alpha.
    float4 final_color = float4(input.color.rgb, alpha * input.color.a);

    return final_color;
}
