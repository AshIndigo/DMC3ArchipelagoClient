cbuffer FontColor : register(b0)
{
    float4 textColor; // RGBA, 0-1
};

Texture2D fontAtlas : register(t0);
SamplerState fontSampler : register(s0);

struct PS_INPUT
{
    float4 pos : SV_POSITION;
    float2 uv : TEXCOORD0;
};

float4 main(PS_INPUT input) : SV_TARGET
{
    float alpha = fontAtlas.Sample(fontSampler, input.uv).a;
    return float4(textColor.rgb, textColor.a * alpha);
}
