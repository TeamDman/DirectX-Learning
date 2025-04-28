struct VS_INPUT
{
    float2 pos : POSITION; // Input vertex position
};

struct VS_OUTPUT
{
    float4 pos : SV_POSITION; // Output clip space position
};

VS_OUTPUT main(VS_INPUT input)
{
    VS_OUTPUT output = (VS_OUTPUT)0;
    // Pass position through, set z=0.5 (between near/far), w=1.0
    output.pos = float4(input.pos.x, input.pos.y, 0.5f, 1.0f);
    return output;
}
