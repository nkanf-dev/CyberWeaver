import { Tldraw, toRichText } from 'tldraw'
import 'tldraw/tldraw.css'
import { invoke } from '@tauri-apps/api/core'
import { useRef } from 'react'

function App() {
  const isLoadingRef = useRef(false);
  const hasLoadedRef = useRef(false);

  const handleMount = (editor: any) => {
    console.log('🚀 Tldraw 已挂载');
    
    const loadFromDB = async () => {
      isLoadingRef.current = true;
      console.log('📂 开始从数据库加载...');
      
      try {
        const nodes: any[] = await invoke('get_nodes');
        console.log(`📦 数据库返回 ${nodes.length} 个节点:`, nodes);
        
        let successCount = 0;
        nodes.forEach((node) => {
          let shapeConfig: any = null;
          
          switch (node.type) {
            case 'geo':
              shapeConfig = {
                id: node.id,
                type: 'geo',
                x: node.x,
                y: node.y,
                props: {
                  geo: 'rectangle',
                  w: 200,
                  h: 100
                }
              };
              break;
              
            case 'text':
              shapeConfig = {
                id: node.id,
                type: 'text',
                x: node.x,
                y: node.y,
                props: {
                  richText: toRichText(node.content || ""),
                  scale: 1
                }
              };
              break;
              
            case 'note':
              shapeConfig = {
                id: node.id,
                type: 'note',
                x: node.x,
                y: node.y,
                props: {
                  richText: toRichText(node.content || ""),
                  color: 'yellow',
                  scale: 1
                }
              };
              break;
              
            default:
              console.warn(`⚠️ 忽略未知类型: ${node.type}`);
              return;
          }
          
          if (shapeConfig) {
            try {
              editor.createShape(shapeConfig);
              successCount++;
              console.log(`✅ 已创建 ${node.type} [ID: ${node.id.substring(0, 20)}...]`);
            } catch (e) {
              console.error(`❌ 创建失败:`, e);
            }
          }
        });
        
        console.log(`✅ 加载完成: ${successCount}/${nodes.length} 个节点成功创建`);
      } catch (e) {
        console.error("❌ 数据库读取失败:", e);
      } finally {
        setTimeout(() => {
          isLoadingRef.current = false;
          hasLoadedRef.current = true;
          console.log('🔓 保存功能已启用');
        }, 500);
      }
    };

    loadFromDB();

    // ========== 保存逻辑 ==========
    let saveTimeout: NodeJS.Timeout | null = null;
    let changeCounter = 0;
    
    editor.store.listen((entry: any) => {
      changeCounter++;
      
      if (isLoadingRef.current) {
        console.log('⏸️ 跳过保存：正在加载中');
        return;
      }
      
      if (!hasLoadedRef.current) {
        console.log('⏸️ 跳过保存：加载锁定期');
        return;
      }
      
      const { updated, added } = entry.changes;
      const allChanges = { ...added, ...updated };
      
      // ✅ 显示所有变化的内容
      console.log(`📝 检测到变化 #${changeCounter}`, {
        总变化数: Object.keys(allChanges).length,
        详细内容: Object.values(allChanges).map((r: any) => ({
          typeName: r.typeName,
          type: r.type,S
          id: r.id?.substring(0, 20)
        }))
      });
      
      if (saveTimeout) {
        clearTimeout(saveTimeout);
      }
      
      saveTimeout = setTimeout(async () => {
        console.log('💾 开始保存流程...');
        console.log('📋 本次变化详情:', allChanges);
        
        let savedCount = 0;
        let skippedCount = 0;
        
        for (const record of Object.values(allChanges) as any[]) {
          console.log(`🔍 检查记录:`, {
            typeName: record.typeName,
            type: record.type,
            id: record.id
          });
          
          // ✅关键 检查为什么跳过
          if (record.typeName !== 'shape') {
            console.log(`❌ 跳过原因: typeName 不是 'shape'，而是 '${record.typeName}'`);
            skippedCount++;
            continue;
          }
          
          const allowedTypes = ['geo', 'text', 'note'];
          
          if (!allowedTypes.includes(record.type)) {
            console.log(`❌ 跳过原因: type '${record.type}' 不在白名单 [${allowedTypes.join(', ')}]`);
            skippedCount++;
            continue;
          }
          
          console.log(`✅ 通过检查，准备保存 ${record.type}`);
          
          // 提取内容
          let content = "";
          
          try {
            if (record.type === 'geo') {
              content = `[${record.props?.geo || 'rectangle'}]`;
            } else if (record.type === 'text' || record.type === 'note') {
              const richText = record.props?.richText;
              
              if (richText) {
                if (typeof richText === 'object' && richText.content) {
                  try {
                    content = richText.content
                      .map((node: any) => {
                        if (node.type === 'paragraph' && node.content) {
                          return node.content
                            .map((textNode: any) => textNode.text || '')
                            .join('');
                        }
                        return '';
                      })
                      .filter(Boolean)
                      .join('\n');
                  } catch (err) {
                    console.warn('richText 解析失败', err);
                    content = JSON.stringify(richText);
                  }
                } else if (typeof richText === 'string') {
                  content = richText;
                }
              }
              
              if (!content && record.props?.text) {
                content = record.props.text;
              }
            }
            
            console.log(`📝 提取的内容: "${content}"`);
          } catch (err) {
            console.error('❌ 内容提取失败:', err);
            content = "[提取失败]";
          }

          const nodeData = {
            id: record.id,
            type: record.type,
            x: record.x,
            y: record.y,
            content: content
          };

          console.log(`📤 调用 Rust 保存:`, nodeData);

          try {
            await invoke('save_node', { node: nodeData });
            savedCount++;
            console.log(`✅✅✅ 保存成功！ [${savedCount}] ${record.type}`);
          } catch (err) {
            console.error(`❌ Rust 调用失败:`, err);
          }
        }
        
        console.log(`📊 统计: 保存 ${savedCount} 个, 跳过 ${skippedCount} 个`);
        
        if (savedCount === 0) {
          console.warn('⚠️ 没有节点被保存！');
        } else {
          console.log(`🎉 成功保存 ${savedCount} 个节点`);
        }
      }, 300);
    });
  };

  return (
    <div style={{ position: 'fixed', inset: 0 }}>
      <div style={{
        position: 'absolute', 
        top: 12, 
        left: '50%', 
        transform: 'translateX(-50%)',
        zIndex: 1000, 
        backgroundColor: '#ff5722', 
        color: '#fff',
        padding: '10px 24px', 
        borderRadius: '8px', 
        fontSize: '15px', 
        fontWeight: '700',
        boxShadow: '0 4px 12px rgba(0,0,0,0.3)'
      }}>
        🔍 调试版 F12 查看为什么没保存
      </div>
      
      <Tldraw onMount={handleMount} />
    </div>
  )
}

export default App
