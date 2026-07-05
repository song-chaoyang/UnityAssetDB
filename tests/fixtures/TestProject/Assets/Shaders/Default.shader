Shader "Hidden/Default"
{
    Properties
    {
        _MainTex ("Texture", 2D) = "white" {}
    }
    SubShader
    {
        Pass
        {
            Name "MainPass"
        }
    }
    Fallback "Diffuse"
}
